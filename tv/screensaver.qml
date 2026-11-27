/* Lounge's screen saver: a clock face, full screen on black, chosen in Lounge's Settings (Analog,
 * Digital, Modular, Numerals). Lounge writes the choice, the clock format and the weather to
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
    property var settings: ({ face: "analog", hour24: false, weather: "" })
    property date now: new Date()

    function load() {
        var request = new XMLHttpRequest()
        request.onreadystatechange = function () {
            if (request.readyState !== XMLHttpRequest.DONE) return
            try {
                var read = JSON.parse(request.responseText)
                if (read && typeof read.face === "string") settings = read
            } catch (error) {}
        }
        request.open("GET", settingsUrl)
        request.send()
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
        visible: settings.face === "analog"
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
            running: true
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
        visible: settings.face === "digital"
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
        visible: settings.face === "modular"
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
                    { label: "WEATHER", value: settings.weather || "—" }
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

    // Numerals: only the quarters, very large, around fine hands, the date under the centre;
    // in the spirit of Apple Watch's Typograph, set in the clock's tall figures.
    Item {
        id: numerals
        visible: settings.face === "numerals"
        width: 980
        height: 980
        x: (window.width - width) / 2 + drift.dx
        y: (window.height - height) / 2 + drift.dy
        readonly property real centre: width / 2
        Repeater {
            model: [
                { n: "12", x: 0, y: -1 }, { n: "3", x: 1, y: 0 },
                { n: "6", x: 0, y: 1 }, { n: "9", x: -1, y: 0 }
            ]
            Text {
                text: modelData.n
                color: Qt.rgba(1, 1, 1, 0.92)
                font.family: tall.name
                font.pixelSize: 250
                width: 320
                height: 260
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                x: numerals.centre + modelData.x * 380 - width / 2
                y: numerals.centre + modelData.y * 360 - height / 2
            }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            y: numerals.centre + 150
            text: weekday(window.now).substring(0, 3).toUpperCase() + " " + window.now.getDate()
            color: "#ff9f0a"
            font.family: figures.name
            font.pixelSize: 44
            font.weight: Font.DemiBold
            font.letterSpacing: 4
        }
        Repeater {
            model: [
                { length: 230, width: 14, minute: false },
                { length: 340, width: 9, minute: true }
            ]
            Item {
                x: numerals.centre
                y: numerals.centre
                rotation: modelData.minute
                    ? (window.now.getMinutes() + window.now.getSeconds() / 60) * 6
                    : ((window.now.getHours() % 12) + window.now.getMinutes() / 60) * 30
                Rectangle {
                    width: modelData.width
                    height: modelData.length
                    radius: modelData.width / 2
                    antialiasing: true
                    x: -modelData.width / 2
                    y: -modelData.length + modelData.width
                    color: "white"
                }
            }
        }
        Item {
            id: numeralsSecond
            x: numerals.centre
            y: numerals.centre
            Rectangle {
                width: 4; radius: 2; antialiasing: true
                height: 420
                x: -2
                y: -370
                color: "#ff9f0a"
            }
        }
        RotationAnimator {
            id: numeralsSweep
            target: numeralsSecond
            duration: 60000
            loops: Animation.Infinite
            running: numerals.visible
            from: 0
            to: 360
            function sync() {
                var at = new Date()
                var angle = (at.getSeconds() + at.getMilliseconds() / 1000) * 6
                stop()
                from = angle
                to = angle + 360
                start()
            }
        }
        onVisibleChanged: if (visible) numeralsSweep.sync()
        Rectangle {
            width: 22; height: 22; radius: 11; antialiasing: true
            color: "#ff9f0a"
            x: numerals.centre - 11; y: numerals.centre - 11
        }
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
