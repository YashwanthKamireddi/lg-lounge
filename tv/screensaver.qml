/* Lounge's screen saver: a clock face, full screen on black, chosen in Lounge's Settings (Analog,
 * Digital, Modular, Numerals, Contour, California, or Shuffle: another each time). Lounge writes the choice, the clock format and the weather to
 * saver.json beside this file's copy; the saver reads it as it starts. webOS draws its screen saver
 * from com.webos.app.screensaver's qml/main.qml; with Lounge as Home `src/home-mount.rs` binds
 * this file over it (its copy in /var/lib/lounge, with `assets` pointing at Lounge's fonts), and
 * takes it off again with Lounge as Home. webOS closes it on any key.
 *
 * Every motion runs on Qt's render thread: the second hand sweeps under a RotationAnimator,
 * resynchronised to the wall clock each minute, and the hour and minute hands step once a
 * second. The clock drifts slowly about the screen so nothing stays on the same pixels.
 * Qt 5.12 (webOS 22) and later.
 */
import QtQuick 2.12
import QtQuick.Window 2.2
import Eos.Window 0.1
import WebOSServices 1.0

WebOSWindow {
    id: window
    width: 1920
    height: 1080
    windowType: "_WEBOS_WINDOW_TYPE_SCREENSAVER"
    appId: "com.webos.app.screensaver"
    color: "black"
    visible: true

    // Lounge's assets folder; the installed copy names the real one.
    property string assets: "file:///media/developer/apps/usr/palm/applications/io.github.yashwanthkamireddi.lounge/assets/"
    // What Lounge last wrote: the face, 12 or 24 hours, and the weather in words.
    property string settingsUrl: "file:///var/lib/lounge/saver.json"
    property var settings: ({ face: "analog", hour24: false, weather: "", weather_until: 0 })
    property date now: new Date()
    // The face on screen: the one chosen, or with Shuffle one of them, drawn as the saver starts.
    readonly property var faces: ["analog", "digital", "modular", "numerals", "contour", "california"]
    property string face: "analog"
    readonly property color orange: "#ff9f0a"

    function load() {
        var request = new XMLHttpRequest()
        request.onreadystatechange = function () {
            if (request.readyState !== XMLHttpRequest.DONE) return
            try {
                var read = JSON.parse(request.responseText)
                if (read && typeof read.face === "string") {
                    settings = read
                    face = read.face === "shuffle"
                        ? faces[Math.floor(Math.random() * faces.length)]
                        : read.face
                }
            } catch (error) {}
        }
        request.open("GET", settingsUrl)
        request.send()
    }

    // The weather Lounge wrote, while it is recent enough to read as now.
    function weatherNow() {
        return (settings.weather_until || 0) * 1000 > window.now.getTime() ? settings.weather : ""
    }
    function weekday(date) {
        return ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][date.getDay()]
    }
    function month(date) {
        return ["January", "February", "March", "April", "May", "June", "July", "August",
                "September", "October", "November", "December"][date.getMonth()]
    }
    // "10:42": no AM/PM beside large figures, as the Lock Screen.
    function time(date) {
        var hours = date.getHours()
        if (!settings.hour24) hours = hours % 12 === 0 ? 12 : hours % 12
        var minutes = date.getMinutes()
        return (settings.hour24 && hours < 10 ? "0" : "") + hours + ":" + (minutes < 10 ? "0" : "") + minutes
    }

    // The digital faces change on each second's tick, and each tick lands just past the wall
    // clock's second: counted from when the saver opened, the minute turned up to a second late.
    Timer {
        interval: 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            window.now = new Date()
            interval = 1010 - window.now.getMilliseconds()
        }
    }

    // Toasts would draw over the saver, as LG's own saver keeps them off.
    Service {
        id: notifications
        appId: "com.webos.app.screensaver"
        function set(state) {
            call("luna://com.webos.notification/", state)
        }
    }
    Component.onCompleted: {
        load()
        notifications.set("disable")
    }
    Component.onDestruction: notifications.set("enable")

    FontLoader {
        id: figures
        source: window.assets + "font-display-600.ttf"
    }
    // Contour's figures: six cuts from narrow and light to wide and heavy (tools/font-weights.py).
    Repeater {
        id: contourCuts
        model: 6
        Item {
            readonly property string family: cut.name
            // Per pixel of font size: how far the figures' ink rises above the baseline, and
            // each figure's width.
            readonly property real inkHeight: Math.max(0.01, metrics.tightBoundingRect("0123456789").height / 100)
            readonly property real inkRise: -metrics.tightBoundingRect("0123456789").y / 100
            function advance(text) { return metrics.advanceWidth(text) / 100 / inkHeight }
            FontLoader {
                id: cut
                source: window.assets + "font-contour-" + index + ".ttf"
            }
            FontMetrics {
                id: metrics
                font.family: cut.name
                font.pixelSize: 100
            }
        }
    }
    // The clock's own tall, narrow figures.
    FontLoader {
        id: tall
        source: window.assets + "font-clock.ttf"
    }

    Item {
        id: clock
        visible: window.face === "analog"
        // Its second hand sweeps only while it is the face on screen.
        onVisibleChanged: visible ? sweep.sync() : sweep.stop()
        // Home's face, larger: a squircle in proportion to the one over the wallpaper.
        readonly property real halfW: 405
        readonly property real halfH: 382
        readonly property color orange: "#ff9f0a"
        width: halfW * 2
        height: halfH * 2
        x: (window.width - width) / 2 + drift.dx
        y: (window.height - height) / 2 + drift.dy
        opacity: 0

        Component.onCompleted: {
            opacity = 1
            hands.set()
            sweep.sync()
        }
        Behavior on opacity {
            NumberAnimation { duration: 900; easing.type: Easing.OutCubic }
        }

        // How far from the centre the outline `inset` inside the face lies, along `angle`
        // (radians clockwise from 12): the squircle Home's clock follows.
        function reach(angle, inset) {
            var a = halfW - inset, b = halfH - inset
            var x = Math.abs(Math.sin(angle) / a), y = Math.abs(Math.cos(angle) / b)
            return Math.pow(Math.pow(x, 4) + Math.pow(y, 4), -1 / 4)
        }

        // The face: near-black glass with a faint lit edge.
        Rectangle {
            anchors.fill: parent
            radius: 244
            antialiasing: true
            gradient: Gradient {
                GradientStop { position: 0; color: "#1b1b1f" }
                GradientStop { position: 1; color: "#0b0b0d" }
            }
            border.width: 2
            border.color: Qt.rgba(1, 1, 1, 0.13)
        }

        // Minute track along the edge, the hours bolder.
        Repeater {
            model: 60
            Item {
                readonly property bool hour: index % 5 === 0
                readonly property real angle: index / 60 * 2 * Math.PI
                x: clock.halfW
                y: clock.halfH
                rotation: index * 6
                Rectangle {
                    width: hour ? 7 : 3
                    height: hour ? 30 : 15
                    radius: width / 2
                    antialiasing: true
                    color: Qt.rgba(1, 1, 1, hour ? 1 : 0.38)
                    x: -width / 2
                    y: -clock.reach(parent.angle, 26)
                }
            }
        }

        // Every hour numbered, upright.
        Repeater {
            model: 12
            Text {
                readonly property real angle: (index + 1) / 12 * 2 * Math.PI
                readonly property real distance: clock.reach(angle, 108)
                text: index + 1
                color: Qt.rgba(1, 1, 1, 0.96)
                font.family: figures.name
                font.pixelSize: 76
                font.weight: Font.DemiBold
                width: 140
                height: 96
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                x: clock.halfW + distance * Math.sin(angle) - width / 2
                y: clock.halfH - distance * Math.cos(angle) - height / 2
            }
        }

        // The date on the dial, as Apple Watch prints it.
        Row {
            id: date
            spacing: 16
            anchors.horizontalCenter: parent.horizontalCenter
            y: clock.halfH + clock.halfH * 0.36
            property date now: new Date()
            Text {
                text: ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"][date.now.getDay()]
                color: clock.orange
                font.family: figures.name
                font.pixelSize: 50
                font.weight: Font.DemiBold
                font.letterSpacing: 3
            }
            Text {
                text: date.now.getDate()
                color: "white"
                font.family: figures.name
                font.pixelSize: 50
                font.weight: Font.DemiBold
                font.letterSpacing: 3
            }
        }

        // The hands: a slim stem out of the centre, then a broad rounded blade, each over a soft
        // shade of itself.
        Item {
            id: hands
            anchors.fill: parent
            property real hourAngle: 0
            property real minuteAngle: 0
            function set() {
                var now = new Date()
                var minutes = now.getMinutes() + now.getSeconds() / 60
                minuteAngle = minutes * 6
                hourAngle = ((now.getHours() % 12) + minutes / 60) * 30
                date.now = now
            }
            Repeater {
                model: [
                    { length: 0.56, blade: 22, shade: true }, { length: 0.86, blade: 16, shade: true },
                    { length: 0.56, blade: 22, shade: false }, { length: 0.86, blade: 16, shade: false }
                ]
                Item {
                    readonly property real length: clock.halfH * modelData.length
                    readonly property color ink: modelData.shade ? Qt.rgba(0, 0, 0, 0.5) : "white"
                    x: clock.halfW
                    y: clock.halfH + (modelData.shade ? 8 : 0)
                    rotation: modelData.length < 0.7 ? hands.hourAngle : hands.minuteAngle
                    Rectangle {
                        width: 8; height: 54; radius: 4; antialiasing: true
                        x: -4; y: -50
                        color: parent.ink
                    }
                    Rectangle {
                        width: modelData.blade; height: parent.length - 46
                        radius: modelData.blade / 2; antialiasing: true
                        x: -modelData.blade / 2; y: -parent.length
                        color: parent.ink
                    }
                }
            }
            Timer {
                interval: 1000
                running: true
                repeat: true
                // On the wall clock's second, as the digital faces' tick.
                onTriggered: {
                    hands.set()
                    interval = 1010 - new Date().getMilliseconds()
                }
            }
        }

        // The centre disc, then the sweeping orange second hand with its counterweight.
        Rectangle {
            width: 30; height: 30; radius: 15; antialiasing: true
            color: "white"
            x: clock.halfW - 15; y: clock.halfH - 15
        }
        Item {
            id: second
            x: clock.halfW
            y: clock.halfH
            Rectangle {
                width: 5; radius: 2.5; antialiasing: true
                height: clock.halfH * 1.14
                x: -2.5
                y: -clock.halfH * 0.92
                color: clock.orange
            }
        }
        RotationAnimator {
            id: sweep
            target: second
            duration: 60000
            loops: Animation.Infinite
            // From the wall clock's second, to the millisecond.
            function sync() {
                var now = new Date()
                var angle = (now.getSeconds() + now.getMilliseconds() / 1000) * 6
                stop()
                from = angle
                to = angle + 360
                start()
            }
        }
        // Each minute the sweep starts again from the wall clock, so it never drifts from it.
        Timer {
            interval: 60000
            property bool running: true
            repeat: true
            onTriggered: sweep.sync()
        }
        Rectangle {
            width: 18; height: 18; radius: 9; antialiasing: true
            color: clock.orange
            x: clock.halfW - 9; y: clock.halfH - 9
        }
        Rectangle {
            width: 7; height: 7; radius: 3.5; antialiasing: true
            color: "#1c1c1e"
            x: clock.halfW - 3.5; y: clock.halfH - 3.5
        }
    }

    // Digital: the Lock Screen's tall figures, the day and date above them.
    Column {
        visible: window.face === "digital"
        spacing: -20
        x: (window.width - width) / 2 + drift.dx
        // The figures' line box keeps room below for descenders, which digits never use:
        // centred on the ink, not the box.
        y: (window.height - height) / 2 + 54 + drift.dy
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: weekday(window.now) + " " + window.now.getDate() + " " + month(window.now)
            color: Qt.rgba(1, 1, 1, 0.86)
            font.family: figures.name
            font.pixelSize: 64
            font.weight: Font.DemiBold
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: time(window.now)
            color: Qt.rgba(1, 1, 1, 0.95)
            font.family: tall.name
            font.pixelSize: 520
            font.letterSpacing: 10
        }
    }

    // Modular: the time large and left aligned over two plain panels, the date and the weather,
    // as Apple Watch's Modular face lays out its complications.
    Item {
        visible: window.face === "modular"
        width: 1300
        height: childrenRect.height
        x: (window.width - width) / 2 + drift.dx
        y: (window.height - height) / 2 + drift.dy
        Text {
            id: modularDay
            text: weekday(window.now).toUpperCase()
            color: "#ff9f0a"
            font.family: figures.name
            font.pixelSize: 52
            font.weight: Font.DemiBold
            font.letterSpacing: 6
        }
        Text {
            id: modularTime
            y: modularDay.height - 40
            text: time(window.now)
            color: "white"
            font.family: tall.name
            font.pixelSize: 440
            font.letterSpacing: 8
        }
        Row {
            y: modularTime.y + modularTime.height - 40
            spacing: 32
            Repeater {
                model: [
                    { label: "DATE", value: window.now.getDate() + " " + month(window.now) },
                    { label: "WEATHER", value: window.weatherNow() || "—" }
                ]
                Rectangle {
                    width: 634
                    height: 170
                    radius: 40
                    antialiasing: true
                    color: "#16161a"
                    border.width: 2
                    border.color: Qt.rgba(1, 1, 1, 0.08)
                    Text {
                        x: 40; y: 28
                        text: modelData.label
                        color: "#ff9f0a"
                        font.family: figures.name
                        font.pixelSize: 30
                        font.weight: Font.DemiBold
                        font.letterSpacing: 4
                    }
                    Text {
                        x: 40; y: 72
                        width: parent.width - 80
                        elide: Text.ElideRight
                        text: modelData.value
                        color: "white"
                        font.family: figures.name
                        font.pixelSize: 60
                        font.weight: Font.DemiBold
                    }
                }
            }
        }
    }

    // The hour, minute and sweeping second hands every hand-drawn face shares, about (cx, cy):
    // a slim stem out of the centre, then a broad rounded blade, each over a soft shade of
    // itself; the second hand sweeps on Qt's render thread, set again from the wall clock each
    // minute so it never drifts. `hour` and `minute` are the blades' lengths.
    // (A Component under a Loader, its settings the Loader's properties: the saver is one file
    // bound over webOS's, so a Hands.qml beside it would not be found.)
    Component {
        id: handsComponent
        Item {
            id: set
            readonly property Item spec: parent
            readonly property color accent: spec && spec.accent !== undefined ? spec.accent : window.orange
            readonly property real minutes: window.now.getMinutes() + window.now.getSeconds() / 60
            readonly property real hourAngle: ((window.now.getHours() % 12) + minutes / 60) * 30
            readonly property real minuteAngle: minutes * 6
            Repeater {
                model: [
                    { hand: "hour", blade: 20, shade: true }, { hand: "minute", blade: 14, shade: true },
                    { hand: "hour", blade: 20, shade: false }, { hand: "minute", blade: 14, shade: false }
                ]
                Item {
                    readonly property real length: set.spec ? set.spec[modelData.hand] : 0
                    readonly property color ink: modelData.shade ? Qt.rgba(0, 0, 0, 0.55)
                        : (set.spec ? set.spec.ink : "white")
                    x: set.spec ? set.spec.cx : 0
                    y: (set.spec ? set.spec.cy : 0) + (modelData.shade ? 8 : 0)
                    rotation: modelData.hand === "hour" ? set.hourAngle : set.minuteAngle
                    Rectangle {
                        width: 8; height: 52; radius: 4; antialiasing: true
                        x: -4; y: -48
                        color: parent.ink
                    }
                    Rectangle {
                        width: modelData.blade; height: parent.length - 44
                        radius: modelData.blade / 2; antialiasing: true
                        x: -modelData.blade / 2; y: -parent.length
                        color: parent.ink
                    }
                }
            }
            Rectangle {
                width: 28; height: 28; radius: 14; antialiasing: true
                color: set.spec ? set.spec.ink : "white"
                x: (set.spec ? set.spec.cx : 0) - 14; y: (set.spec ? set.spec.cy : 0) - 14
            }
            Item {
                id: secondHand
                x: set.spec ? set.spec.cx : 0
                y: set.spec ? set.spec.cy : 0
                Rectangle {
                    width: 4; radius: 2; antialiasing: true
                    height: (set.spec ? set.spec.second : 0) * 1.18
                    x: -2
                    y: -(set.spec ? set.spec.second : 0)
                    color: set.accent
                }
                Rectangle {
                    width: 16; height: 16; radius: 8; antialiasing: true
                    color: set.accent
                    x: -8; y: -8
                }
                Rectangle {
                    width: 6; height: 6; radius: 3; antialiasing: true
                    color: "#1c1c1e"
                    x: -3; y: -3
                }
            }
            RotationAnimator {
                id: sweepHand
                target: secondHand
                duration: 60000
                loops: Animation.Infinite
                function sync() {
                    var at = new Date()
                    var angle = (at.getSeconds() + at.getMilliseconds() / 1000) * 6
                    stop()
                    from = angle
                    to = angle + 360
                    start()
                }
            }
            readonly property bool running: set.spec ? set.spec.running : false
            onRunningChanged: running ? sweepHand.sync() : sweepHand.stop()
            Component.onCompleted: if (running) sweepHand.sync()
            Timer {
                interval: 60000
                running: set.running
                repeat: true
                onTriggered: sweepHand.sync()
            }
        }
    }

    // Numerals: every hour in the clock's tall figures, round fine hands, as Apple Watch's
    // Typograph sets them; the date under the centre.
    Item {
        id: numerals
        visible: window.face === "numerals"
        width: window.width
        height: window.height
        x: drift.dx
        y: drift.dy
        Repeater {
            model: 12
            Text {
                readonly property real angle: (index + 1) / 12 * 2 * Math.PI
                text: index + 1
                color: Qt.rgba(1, 1, 1, 0.94)
                font.family: tall.name
                font.pixelSize: 176
                width: 260
                height: 190
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                x: window.width / 2 + 392 * Math.sin(angle) - width / 2
                y: window.height / 2 - 392 * Math.cos(angle) - height / 2
            }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            y: window.height / 2 + 120
            text: weekday(window.now).substring(0, 3).toUpperCase() + " " + window.now.getDate()
            color: window.orange
            font.family: figures.name
            font.pixelSize: 40
            font.weight: Font.DemiBold
            font.letterSpacing: 4
        }
        Loader {
            sourceComponent: handsComponent
            property real cx: window.width / 2
            property real cy: window.height / 2
            property real hour: 200
            property real minute: 300
            property real second: 330
            property bool running: numerals.visible
            property color ink: "white"
        }
    }

    // Contour, after Apple Watch's, across a TV's whole screen. The twelve hours run as one
    // band round the screen's edge, each figure lying along its edge as on the watch (upright
    // along the top, on its side down the sides, upside down along the bottom, turning with
    // the corners), its outer side on the band's outer line. The hours about the hour hand
    // grow large and wide (the widest, heaviest of six cuts), the far side of the dial stays
    // small and narrow, and the band turns so the hand's hour lies where the hand points. As
    // the hand moves the swell moves with it. Cream on black, white hands, a red second hand.
    Item {
        id: contour
        visible: window.face === "contour"
        width: window.width
        height: window.height
        x: drift.dx
        y: drift.dy
        readonly property color ink: "#f4dfb6"
        // The band's outer line: a rounded rectangle, half sizes `a` by `b`, corner radius `r`.
        readonly property real a: window.width / 2 - 40
        readonly property real b: window.height / 2 - 40
        readonly property real r: 260
        // How tall the hand's hour stands and how tall the farthest; the black between two.
        readonly property real tallest: 270
        readonly property real smallest: 150
        readonly property real gap: 30
        readonly property real perimeter: 4 * (a - r) + 4 * (b - r) + 2 * Math.PI * r
        readonly property int minuteOfDay: window.now.getHours() * 60 + window.now.getMinutes()
        // The figures settle into place before they start to glide (else they fly in from 0).
        property bool settled: false
        Timer { interval: 100; running: contour.visible; onTriggered: contour.settled = true }

        // The point `s` along the outer line, clockwise from the top centre, and which way the
        // line runs there (degrees, 0 to the right).
        function along(s) {
            var w = a - r, h = b - r, arc = Math.PI * r / 2
            s = ((s % perimeter) + perimeter) % perimeter
            if (s < w) return { x: s, y: -b, angle: 0 }
            s -= w
            if (s < arc) { var q1 = -Math.PI / 2 + s / r
                return { x: w + r * Math.cos(q1), y: -h + r * Math.sin(q1), angle: q1 * 180 / Math.PI + 90, curved: true } }
            s -= arc
            if (s < 2 * h) return { x: a, y: -h + s, angle: 90 }
            s -= 2 * h
            if (s < arc) { var q2 = s / r
                return { x: w + r * Math.cos(q2), y: h + r * Math.sin(q2), angle: q2 * 180 / Math.PI + 90, curved: true } }
            s -= arc
            if (s < 2 * w) return { x: w - s, y: b, angle: 180 }
            s -= 2 * w
            if (s < arc) { var q3 = Math.PI / 2 + s / r
                return { x: -w + r * Math.cos(q3), y: h + r * Math.sin(q3), angle: q3 * 180 / Math.PI + 90, curved: true } }
            s -= arc
            if (s < 2 * h) return { x: -a, y: h - s, angle: 270 }
            s -= 2 * h
            if (s < arc) { var q4 = Math.PI + s / r
                return { x: -w + r * Math.cos(q4), y: -h + r * Math.sin(q4), angle: q4 * 180 / Math.PI + 90, curved: true } }
            s -= arc
            return { x: -w + s, y: -b, angle: 360 }
        }
        // Where along the line a hand at `degrees` (clockwise from 12) meets it.
        function pointedAt(degrees) {
            var best = 0, bestApart = 360
            for (var i = 0; i < 720; i++) {
                var s = i / 720 * perimeter, spot = along(s)
                var heading = (Math.atan2(spot.x, -spot.y) * 180 / Math.PI + 360) % 360
                var apart = Math.abs(heading - degrees)
                apart = Math.min(apart, 360 - apart)
                if (apart < bestApart) { bestApart = apart; best = s }
            }
            return best
        }
        // Each hour: its cut, its size (the height its figures' ink stands), its place along the
        // line and its turn. Sizes and cuts follow a bell over the dial about the hour hand
        // (wrapping past 12); the figures are laid end to end round the line, the room left
        // shared out evenly between them, and the band turned so the hand's hour is centred
        // where the hand points.
        readonly property var layout: {
            var cuts = contourCuts.count === 6 ? contourCuts : null
            if (!cuts) return []
            var hand = (minuteOfDay % 720) / 60
            var hours = [], used = 0
            for (var i = 0; i < 12; i++) {
                var apart = Math.abs(i - hand)
                apart = Math.min(apart, 12 - apart)
                var near = Math.exp(-apart * apart / 2.2)
                var cut = Math.round(near * 5)
                var item = cuts.itemAt(cut)
                if (!item) return []
                var size = smallest + (tallest - smallest) * near
                var text = i === 0 ? "12" : String(i)
                var length = item.advance(text) * size
                hours.push({ text: text, cut: cut, size: size, length: length, near: near })
                used += length
            }
            var spacing = Math.max(gap, (perimeter - used) / 12)
            var middles = [], at = 0
            for (var j = 0; j < 12; j++) {
                middles.push(at + hours[j].length / 2)
                at += hours[j].length + spacing
            }
            // The hand's place between its two hours, along the band.
            var before = Math.floor(hand) % 12, after = (before + 1) % 12, part = hand - Math.floor(hand)
            var ahead = middles[after] < middles[before] ? middles[after] + perimeter : middles[after]
            var onBand = middles[before] + (ahead - middles[before]) * part
            var shift = pointedAt(hand * 30) - onBand
            for (var k = 0; k < 12; k++) {
                var spot = along(middles[k] + shift)
                // On a corner's curve a figure's ends would reach past the line (and the
                // screen): it stands in by how far.
                var chord = Math.min(hours[k].length, 2 * r)
                var inward = spot.curved ? chord * chord / (8 * r) : 0
                var turn = spot.angle * Math.PI / 180
                hours[k].x = spot.x - Math.sin(turn) * inward
                hours[k].y = spot.y + Math.cos(turn) * inward
                hours[k].angle = spot.angle
            }
            return hours
        }
        Repeater {
            model: contour.layout.length
            Item {
                readonly property var hour: contour.layout[index]
                readonly property var cut: contourCuts.itemAt(hour.cut)
                x: window.width / 2 + hour.x
                y: window.height / 2 + hour.y
                rotation: hour.angle
                Behavior on x { enabled: contour.settled; NumberAnimation { duration: 1200; easing.type: Easing.InOutCubic } }
                Behavior on y { enabled: contour.settled; NumberAnimation { duration: 1200; easing.type: Easing.InOutCubic } }
                Behavior on rotation { enabled: contour.settled; RotationAnimation { duration: 1200; easing.type: Easing.InOutCubic; direction: RotationAnimation.Shortest } }
                Text {
                    text: hour.text
                    color: contour.ink
                    font.family: cut ? cut.family : figures.name
                    // Sized so its ink stands `hour.size`; its ink's top on the outer line.
                    font.pixelSize: cut ? hour.size / cut.inkHeight : hour.size
                    x: -contentWidth / 2
                    y: -(baselineOffset - (cut ? cut.inkRise : 0.72) * font.pixelSize)
                }
            }
        }
        Loader {
            sourceComponent: handsComponent
            property real cx: window.width / 2
            property real cy: window.height / 2
            property real hour: 200
            property real minute: 300
            property real second: 320
            property bool running: contour.visible
            property color ink: "white"
            property color accent: "#ff3b30"
        }
    }

    // California, as Apple Watch's full-screen California dial with Arabic figures: a triangle
    // at 12, bars at 3, 6 and 9, the other hours in figures, and a minute track round the
    // screen's edge; each mark where a hand pointing at it meets the
    // edge, so the hands read true. Warm ivory on black, the second hand red.
    Item {
        id: california
        visible: window.face === "california"
        width: window.width
        height: window.height
        x: drift.dx
        y: drift.dy
        readonly property color ivory: "#f1e7d0"
        // The minute track, the hours longer, along the screen's edge.
        Repeater {
            model: 60
            Item {
                readonly property bool hour: index % 5 === 0
                readonly property real angle: index / 60 * 2 * Math.PI
                readonly property real distance: window.edge(angle, 900, 482, 10)
                x: window.width / 2 + distance * Math.sin(angle)
                y: window.height / 2 - distance * Math.cos(angle)
                rotation: index * 6
                Rectangle {
                    width: hour ? 6 : 3
                    height: hour ? 30 : 16
                    radius: width / 2
                    antialiasing: true
                    x: -width / 2
                    y: 0
                    color: hour ? california.ivory : Qt.rgba(0.95, 0.91, 0.82, 0.4)
                }
            }
        }
        Repeater {
            model: ["1", "2", "bar", "4", "5", "bar", "7", "8", "bar", "10", "11", "triangle"]
            Item {
                readonly property real angle: (index + 1) / 12 * 2 * Math.PI
                readonly property real distance: window.edge(angle, 770, 372, 10)
                x: window.width / 2 + distance * Math.sin(angle)
                y: window.height / 2 - distance * Math.cos(angle)
                Text {
                    visible: modelData !== "bar" && modelData !== "triangle"
                    text: modelData
                    color: california.ivory
                    font.family: figures.name
                    font.pixelSize: 124
                    font.weight: Font.DemiBold
                    width: 300
                    height: 150
                    x: -width / 2
                    y: -height / 2
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                // 3, 6 and 9: a bar along the hand that points at it.
                Rectangle {
                    visible: modelData === "bar"
                    width: 104
                    height: 24
                    radius: 12
                    antialiasing: true
                    x: -width / 2
                    y: -height / 2
                    rotation: (index + 1) * 30 - 90
                    color: california.ivory
                }
                // 12: a triangle pointing in.
                Canvas {
                    visible: modelData === "triangle"
                    width: 104
                    height: 92
                    x: -width / 2
                    y: -height / 2
                    antialiasing: true
                    onPaint: {
                        var context = getContext("2d")
                        context.reset()
                        context.fillStyle = california.ivory
                        context.beginPath()
                        context.moveTo(0, 0)
                        context.lineTo(width, 0)
                        context.lineTo(width / 2, height)
                        context.closePath()
                        context.fill()
                    }
                }
            }
        }
        Loader {
            sourceComponent: handsComponent
            property real cx: window.width / 2
            property real cy: window.height / 2
            property real hour: 200
            property real minute: 300
            property real second: 340
            property color ink: california.ivory
            property bool running: california.visible
            property color accent: "#ff3b30"
        }
    }

    // How far from the centre a rounded rectangle of half sizes `a` by `b` lies along `angle`
    // (radians clockwise from 12): a superellipse of power `power`, its corners as round as a
    // TV's picture's.
    function edge(angle, a, b, power) {
        var x = Math.abs(Math.sin(angle)) / a, y = Math.abs(Math.cos(angle)) / b
        return Math.pow(Math.pow(x, power) + Math.pow(y, power), -1 / power)
    }

    // A drift of a few pixels about the exact centre: the face reads as centred, as a watch face
    // is, while no pixel holds the same light all night. (LG's own pixel shift guards OLED panels
    // as well.) It steps once a minute, a pixel or two at a time, along a slow closed path: a
    // continuous animation redrew the whole saver every frame all night, even on the faces that
    // otherwise change once a second.
    QtObject {
        id: drift
        property real dx: 0
        property real dy: 0
        property int minute: 0
        function step() {
            minute += 1
            dx = Math.round(8 * Math.sin(minute * 2 * Math.PI / 36))
            dy = Math.round(6 * Math.sin(minute * 2 * Math.PI / 54))
        }
    }
    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: drift.step()
    }
}
