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

    // Contour, after Apple Watch's, across a TV's whole screen. The hours run as a band round
    // the screen's edge, every figure upright: 11, 12 and 1 along the top, 2, 3 and 4 down the
    // right, 7, 6 and 5 along the bottom, 8, 9 and 10 up the left, a two-figure hour's digits
    // following the band (side by side along the top and bottom, one above the other down the
    // sides). Each figure fills its stretch of the band as the watch's do: about the hour
    // hand the band is deep and the figures bold, and toward the far side it thins until the
    // figures are fine lines. Drawn as rounded strokes of even weight (`digitLines`), so a
    // figure fills any shape without its strokes stretching. Cream on black, white hands, a
    // red second hand.
    Item {
        id: contour
        visible: window.face === "contour"
        width: window.width
        height: window.height
        x: drift.dx
        y: drift.dy
        readonly property color ink: "#f4dfb6"
        // The band's outer edge, half sizes `a` by `b`; how deep it is at the hand's hour and
        // at the far side; the black between two figures, and between two digits.
        readonly property real a: window.width / 2 - 44
        readonly property real b: window.height / 2 - 44
        readonly property real deepest: 230
        readonly property real thinnest: 58
        readonly property real gap: 26
        readonly property int minuteOfDay: window.now.getHours() * 60 + window.now.getMinutes()
        // Every digit's box (relative to the screen's centre), its figure and stroke weight.
        readonly property var digits: {
            var hand = (minuteOfDay % 720) / 60
            var near = [], depth = []
            for (var i = 0; i < 12; i++) {
                var apart = Math.abs(i - hand)
                apart = Math.min(apart, 12 - apart)
                near.push(Math.exp(-apart * apart / 2.6))
                depth.push(thinnest + (deepest - thinnest) * near[i])
            }
            // Each edge: its hours in reading order, the stretch they share (from, to), and how
            // a box is made from a stretch and a depth. Top and bottom stop short of the
            // corners by the depth of the side figure there, so no two figures meet.
            var edges = [
                { hours: [11, 0, 1], from: -a + depth[10] + gap, to: a - depth[2] - gap, across: true,
                  box: function (s, t, d) { return { x: s, y: -b, w: t - s, h: d } } },
                { hours: [2, 3, 4], from: -b, to: b, across: false,
                  box: function (s, t, d) { return { x: a - d, y: s, w: d, h: t - s } } },
                { hours: [7, 6, 5], from: -a + depth[8] + gap, to: a - depth[4] - gap, across: true,
                  box: function (s, t, d) { return { x: s, y: b - d, w: t - s, h: d } } },
                { hours: [10, 9, 8], from: -b, to: b, across: false,
                  box: function (s, t, d) { return { x: -a, y: s, w: d, h: t - s } } }
            ]
            var placed = []
            for (var e = 0; e < 4; e++) {
                var edge = edges[e], weights = [], sum = 0
                for (var j = 0; j < 3; j++) {
                    var hour = edge.hours[j]
                    var weight = (hour === 0 || hour >= 10 ? 1.45 : 1) * (0.55 + 1.6 * near[hour])
                    weights.push(weight)
                    sum += weight
                }
                var at = edge.from, room = edge.to - edge.from - 2 * gap
                for (var k = 0; k < 3; k++) {
                    var hour2 = edge.hours[k], length = weights[k] / sum * room
                    var text = hour2 === 0 ? "12" : String(hour2)
                    // A two-figure hour: its digits share the stretch, a "1" taking less.
                    var shares = [], total = 0
                    for (var c = 0; c < text.length; c++) {
                        shares.push(text[c] === "1" ? 0.4 : 1)
                        total += shares[c]
                    }
                    var between = depth[hour2] * 0.16 * (text.length - 1)
                    // A figure stretches only so far before it stops reading as itself: a
                    // digit at most twice as wide as the band is deep along the top and bottom,
                    // at most two and a half times as tall as it is deep down the sides. A
                    // figure with more room than that sits in the middle of its stretch.
                    var most = depth[hour2] * (edge.across ? 2 : 2.6) * total + between
                    var used = Math.min(length, most)
                    var start = at + (length - used) / 2
                    for (var d = 0; d < text.length; d++) {
                        var part = (used - between) * shares[d] / total
                        var box = edge.box(start, start + part, depth[hour2])
                        box.digit = text[d]
                        box.weight = Math.max(4, Math.min(box.w, box.h) * (0.09 + 0.17 * near[hour2]))
                        placed.push(box)
                        start += part + depth[hour2] * 0.16
                    }
                    at += length + gap
                }
            }
            return placed
        }
        Repeater {
            model: contour.digits.length
            Canvas {
                readonly property var box: contour.digits[index]
                x: window.width / 2 + box.x
                y: window.height / 2 + box.y
                width: box.w
                height: box.h
                antialiasing: true
                onBoxChanged: requestPaint()
                onPaint: window.digitLines(getContext("2d"), box.digit, width, height, box.weight, contour.ink)
            }
        }
        Loader {
            sourceComponent: handsComponent
            property real cx: window.width / 2
            property real cy: window.height / 2
            property real hour: 190
            property real minute: 270
            property real second: 290
            property bool running: contour.visible
            property color ink: "white"
            property color accent: "#ff3b30"
        }
    }

    // Draws `digit` as rounded strokes of `weight` filling a `width` by `height` box: square
    // figures with round corners, as Contour's, built from polylines whose corners are rounded
    // as much as their sides allow, so the figure keeps its weight whatever the box's shape.
    function digitLines(context, digit, width, height, weight, ink) {
        context.reset()
        var p = weight / 2, L = p, T = p, R = width - p, B = height - p, M = height / 2
        // A 7's corner turns into a diagonal: rounded as much, it would read as a hook.
        var round = Math.max(0, Math.min(R - L, B - T) * (digit === "7" ? 0.12 : 0.45))
        var one = Math.min((R - L) / 2, (B - T) * 0.2)
        var middle = (L + R) / 2
        var shapes = {
            "0": [[[L, T], [R, T], [R, B], [L, B]], true],
            "1": [[[middle - one, T + one * 0.8], [middle, T]], [[middle, T], [middle, B]]],
            "2": [[[L, T], [R, T], [R, M], [L, M], [L, B], [R, B]]],
            "3": [[[L, T], [R, T], [R, B], [L, B]], [[L + (R - L) * 0.3, M], [R, M]]],
            "4": [[[L, T], [L, M], [R, M]], [[R, T], [R, B]]],
            "5": [[[R, T], [L, T], [L, M], [R, M], [R, B], [L, B]]],
            "6": [[[R, T], [L, T], [L, B], [R, B], [R, M], [L, M]]],
            "7": [[[L, T], [R, T], [L + (R - L) * 0.3, B]]],
            "8": [[[L, T], [R, T], [R, B], [L, B]], true, [[L, M], [R, M]]],
            "9": [[[R, M], [L, M], [L, T], [R, T], [R, B], [L, B]]]
        }[digit] || []
        context.strokeStyle = ink
        context.lineWidth = weight
        context.lineCap = "round"
        context.lineJoin = "round"
        context.beginPath()
        for (var i = 0; i < shapes.length; i++) {
            if (shapes[i] === true) continue
            var points = shapes[i], closed = shapes[i + 1] === true
            var corner = function (k) { return points[(k + points.length) % points.length] }
            var radius = function (k) {
                var a = corner(k - 1), b = corner(k), c = corner(k + 1)
                var before = Math.abs(b[0] - a[0]) + Math.abs(b[1] - a[1])
                var after = Math.abs(c[0] - b[0]) + Math.abs(c[1] - b[1])
                return Math.max(0, Math.min(round, before / 2, after / 2))
            }
            if (closed) {
                var first = corner(0), second = corner(1)
                context.moveTo((first[0] + second[0]) / 2, (first[1] + second[1]) / 2)
                for (var k = 1; k <= points.length; k++)
                    context.arcTo(corner(k)[0], corner(k)[1], corner(k + 1)[0], corner(k + 1)[1], radius(k))
                context.closePath()
            } else {
                context.moveTo(points[0][0], points[0][1])
                for (var m = 1; m < points.length - 1; m++)
                    context.arcTo(points[m][0], points[m][1], points[m + 1][0], points[m + 1][1], radius(m))
                context.lineTo(points[points.length - 1][0], points[points.length - 1][1])
            }
        }
        context.stroke()
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
