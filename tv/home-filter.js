/* Lounge's compositor key filter (executable Home method; `lounge --home enable` registers it in
 * com.webos.surfacemanager.keyFilters). LG's compositor loads it with Qt.include within seconds of
 * a cold boot, long before the Homebrew hooks (~40 s). While the TV boots it asks lounge-early
 * (com.webos.service.loungehome, src/bin/lounge-early.rs) to mount Lounge over LG's Home
 * executable, then opens Home, at about 20 s. Keys pass through unchanged. ES5 for LG's QML
 * engine.
 */
var loungeHomeBoot = ''; // "" until bootd answers, then "booting" or "done"
var loungeHomeEarly = ''; // "" until lounge-early answers, then "ready" or "failed"
var loungeHomeTries = 0;
var loungeHomeFront = '';
var loungeHomeLaunches = 0;
// A wake from standby (Quick Start+) is not a boot: webOS reopens the last input and LG's Home
// puts itself over it when "Home Auto Launch" is on. True for the few seconds after a wake.
var loungeHomeWaking = false;
var loungeHomePower = '';
var loungeHomeAuto = true; // the TV's "Home Auto Launch" setting
// The selected Home target; the native helper (src/home-mount.rs) writes it into the installed copy.
var loungeHomeTarget = 'com.webos.app.home';

function loungeHomeParse(payload) {
  return typeof payload === 'string' ? JSON.parse(payload) : payload;
}

function loungeHomePrepare() {
  if (loungeHomeEarly || loungeHomeBoot !== 'booting' || loungeHomeTries >= 30) return;
  loungeHomeTries++;
  // ls-hubd routes the call only after its volatile scan at core-boot-done, and an unroutable
  // call gets no reply: ask again on a timer.
  genericTimer.set(1500, loungeHomePrepare);
  lunaCall.send('com.webos.service.loungehome', '/prepare', '{}', function (payload) {
    try {
      var r = loungeHomeParse(payload);
      if (loungeHomeEarly || !r || typeof r.exitCode !== 'number') return;
      loungeHomeEarly = r.returnValue === true ? 'ready' : 'failed';
      loungeHomeOpen();
    } catch (error) {
      console.warn('lounge home filter: ' + error);
    }
  });
}

// Home is a system app: SAM takes its launch at once, while normal apps wait for
// minimal-boot-done. webOS's boot manager opens its first source (the last input, through
// inputcommon) a moment after core-boot-done: opened before it, Home was covered by it ~0.3 s
// later. So Home opens once that input is in front (or after 3 s without one), and again each
// time an input comes over it until the boot is done. An app someone opened stays.
var loungeHomeWaitOver = false;
var loungeHomeWaiting = false;

function loungeHomeIsInput(id) {
  return /^com\.webos\.app\.(livetv|inputcommon|hdmi\d|externalinput\..*)$/.test(id);
}

function loungeHomeOpen() {
  var booting = loungeHomeEarly === 'ready' && loungeHomeBoot === 'booting';
  if (
    !loungeHomeAuto ||
    (!booting && !loungeHomeWaking) ||
    loungeHomeFront === loungeHomeTarget ||
    loungeHomeLaunches >= 4
  )
    return;
  var input = loungeHomeIsInput(loungeHomeFront);
  if (loungeHomeFront && !input) return;
  if (!input && !loungeHomeWaitOver) {
    if (!loungeHomeWaiting) {
      loungeHomeWaiting = true;
      genericTimer.set(3000, function () {
        loungeHomeWaitOver = true;
        loungeHomeOpen();
      });
    }
    return;
  }
  loungeHomeLaunches++;
  lunaCall.send(
    'com.webos.applicationManager',
    '/launch',
    JSON.stringify({ id: loungeHomeTarget }),
    function () {},
  );
}

// Each wake from standby opens Home again over the input webOS reopens, as LG's Home does.
// Off is standby or suspend; the screen turned off or the screen saver is not.
function loungeHomeIsOff(state) {
  return /standby|suspend|power off/i.test(state);
}

function loungeHomePowerChanged(state) {
  var was = loungeHomePower;
  loungeHomePower = state;
  // Powering off: the next cold boot opens Home first, never the input watched last.
  if (loungeHomeIsOff(state) && was && !loungeHomeIsOff(was)) {
    lunaCall.send('com.webos.service.loungehome', '/standby', '{}', function () {});
    return;
  }
  if (state !== 'Active' || !loungeHomeIsOff(was)) return;
  loungeHomeWaking = true;
  loungeHomeLaunches = 0;
  loungeHomeWaitOver = false;
  loungeHomeWaiting = false;
  genericTimer.set(15000, function () {
    loungeHomeWaking = false;
  });
  loungeHomeOpen();
}

// Called by LG's compositor once the file is loaded.
// oxlint-disable-next-line no-unused-vars
function init() {
  try {
    lunaCall.subscribe(
      'com.webos.bootManager',
      '/getBootStatus',
      '{}',
      function (payload) {
        try {
          var r = loungeHomeParse(payload);
          if (!r || !r.returnValue || !r.signals) return;
          // Only a cold boot: reloading the filter while the TV is in use never takes the screen.
          loungeHomeBoot = r.signals['boot-done']
            ? 'done'
            : loungeHomeBoot === ''
              ? 'booting'
              : loungeHomeBoot;
          loungeHomePrepare();
        } catch (error) {
          console.warn('lounge home filter: ' + error);
        }
      },
      true,
    );
    lunaCall.subscribe(
      'com.webos.applicationManager',
      '/getForegroundAppInfo',
      '{}',
      function (payload) {
        try {
          var r = loungeHomeParse(payload);
          loungeHomeFront = (r && r.appId) || '';
          loungeHomeOpen();
        } catch (error) {
          console.warn('lounge home filter: ' + error);
        }
      },
      true,
    );
    lunaCall.subscribe(
      'com.webos.service.tvpower',
      '/power/getPowerState',
      '{}',
      function (payload) {
        try {
          var r = loungeHomeParse(payload);
          if (r && typeof r.state === 'string') loungeHomePowerChanged(r.state);
        } catch (error) {
          console.warn('lounge home filter: ' + error);
        }
      },
      true,
    );
    lunaCall.subscribe(
      'com.webos.settingsservice',
      '/getSystemSettings',
      '{"category":"general","keys":["homeAutoLaunch"]}',
      function (payload) {
        try {
          var r = loungeHomeParse(payload);
          var auto = r && r.settings && r.settings.homeAutoLaunch;
          if (typeof auto === 'string') loungeHomeAuto = auto !== 'off';
        } catch (error) {
          console.warn('lounge home filter: ' + error);
        }
      },
      true,
    );
  } catch (error) {
    console.warn('lounge home filter: ' + error);
  }
}

// The handler named in the key filter chain (src/home-mount.rs); keys pass through.
// oxlint-disable-next-line no-unused-vars
function loungeHomeKey() {
  return KeyPolicy.NextPolicy;
}
