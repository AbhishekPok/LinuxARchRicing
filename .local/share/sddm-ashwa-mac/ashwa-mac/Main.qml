// macOS-style login window. Mirrors amane's lock screen
// (~/.config/amane/src/lock.rs): same wallpaper and shading, date + big clock
// up top, avatar / name / frosted password pill at the bottom, and round
// Sleep / Restart / Shut Down buttons that need a second click within 3 s.
// Sizes follow the screen height (u = height / 1080), as the lock's do.
import QtQuick
import QtQuick.Controls
import Qt5Compat.GraphicalEffects
import SddmComponents 2.0

Rectangle {
    id: root
    width: 1920
    height: 1080
    color: "black"

    readonly property real u: height / 1080
    readonly property string sans: "Inter"
    readonly property string icons: "Material Symbols Rounded"
    readonly property int usernameRole: Qt.UserRole + 1
    readonly property int realNameRole: Qt.UserRole + 2
    property int userIndex: userModel.lastIndex >= 0 ? userModel.lastIndex : 0
    property int sessionIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
    readonly property string userName: userModel.data(userModel.index(userIndex, 0), usernameRole) || userModel.lastUser
    readonly property string realName: userModel.data(userModel.index(userIndex, 0), realNameRole) || ""
    property bool checking: false
    property bool failed: false
    property string armed: ""

    function white(a) { return Qt.rgba(1, 1, 1, a / 255) }

    function submit() {
        if (password.text.length === 0 || checking) return
        checking = true
        failed = false
        sddm.login(userName, password.text, sessionIndex)
    }

    Connections {
        target: sddm
        function onLoginFailed() {
            checking = false
            failed = true
            password.text = ""
            password.forceActiveFocus()
            shake.restart()
        }
        function onLoginSucceeded() { checking = false }
    }

    // ── backdrop: sharp wallpaper, shaded top and bottom for legibility ──
    Image {
        id: wall
        anchors.fill: parent
        source: "background.jpg"
        fillMode: Image.PreserveAspectCrop
        asynchronous: false
        scale: 1.05 - 0.05 * entry.back
        opacity: entry.back
    }
    Rectangle {
        anchors.fill: parent
        opacity: entry.back
        gradient: Gradient {
            GradientStop { position: 0.0;  color: Qt.rgba(0, 0, 0, 0x8c / 255) }
            GradientStop { position: 0.32; color: Qt.rgba(0, 0, 0, 0x26 / 255) }
            GradientStop { position: 0.62; color: Qt.rgba(0, 0, 0, 0x1a / 255) }
            GradientStop { position: 1.0;  color: Qt.rgba(0, 0, 0, 0xa6 / 255) }
        }
    }

    // staged entry like the lock: backdrop up first, content rises in behind it
    QtObject {
        id: entry
        property real back: 0
        property real content: 0
    }
    SequentialAnimation {
        running: true
        NumberAnimation { target: entry; property: "back"; to: 1; duration: 260; easing.type: Easing.OutCubic }
    }
    SequentialAnimation {
        running: true
        PauseAnimation { duration: 140 }
        NumberAnimation { target: entry; property: "content"; to: 1; duration: 480; easing.type: Easing.OutQuint }
    }
    readonly property real rise: 24 * u * (1 - entry.content)

    // ── top right: keyboard layout, like the menu-bar cluster on the lock ──
    Text {
        anchors.right: parent.right
        anchors.rightMargin: 24 * u
        y: 18 * u + (30 * u - height) / 2
        opacity: entry.content
        font.family: sans
        font.pixelSize: 14 * u
        font.weight: Font.Medium
        color: white(0xd9)
        text: keyboard.layouts.length > 0 && keyboard.currentLayout < keyboard.layouts.length
              ? keyboard.layouts[keyboard.currentLayout].shortName.toUpperCase() : "US"
    }

    // ── top: date and the big clock ──
    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.height * 0.075 - rise
        opacity: entry.content
        spacing: 10 * u

        Text {
            id: date
            anchors.horizontalCenter: parent.horizontalCenter
            font.family: sans
            font.pixelSize: 26 * u
            font.weight: Font.DemiBold
            color: white(0xe6)
        }
        Text {
            id: clock
            anchors.horizontalCenter: parent.horizontalCenter
            font.family: sans
            font.pixelSize: 150 * u
            font.weight: Font.Bold
            lineHeight: 0.82
            color: white(0xeb)
        }
        Timer {
            interval: 1000
            running: true
            repeat: true
            triggeredOnStart: true
            onTriggered: {
                var now = new Date()
                date.text = Qt.formatDate(now, "dddd d MMMM")
                clock.text = Qt.formatTime(now, "HH:mm")
            }
        }
    }

    // ── bottom: avatar, name, password pill, hint ──
    Column {
        id: account
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.height - 128 * u - height + rise
        opacity: entry.content
        spacing: 12 * u

        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 84 * u
            height: 84 * u
            Image {
                id: face
                anchors.fill: parent
                source: "face.png"
                fillMode: Image.PreserveAspectCrop
                smooth: true
                visible: false
            }
            Rectangle {
                id: faceMask
                anchors.fill: parent
                radius: width / 2
                visible: false
            }
            OpacityMask {
                anchors.fill: parent
                source: face
                maskSource: faceMask
                visible: face.status === Image.Ready
            }
            // no picture: the initial on a blue gradient, as the lock draws it
            Rectangle {
                anchors.fill: parent
                radius: width / 2
                visible: face.status !== Image.Ready
                gradient: Gradient {
                    GradientStop { position: 0; color: "#2f95ff" }
                    GradientStop { position: 1; color: "#50a8cc" }
                }
                Text {
                    anchors.centerIn: parent
                    text: userName.length ? userName.charAt(0).toUpperCase() : "?"
                    font.family: sans
                    font.pixelSize: 38 * u
                    font.weight: Font.DemiBold
                    color: "white"
                }
            }
            Rectangle {
                anchors.fill: parent
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: white(0x40)
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: realName.length ? realName : userName
            font.family: sans
            font.pixelSize: 19 * u
            font.weight: Font.DemiBold
            color: "white"
        }

        Item {
            id: fieldBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 260 * u
            height: 38 * u

            // frosted: a blurred crop of the wallpaper under a white tint
            Item {
                id: frost
                x: field.x
                width: parent.width
                height: parent.height
                layer.enabled: true
                layer.effect: OpacityMask {
                    maskSource: Rectangle { width: frost.width; height: frost.height; radius: frost.height / 2 }
                }
                ShaderEffectSource {
                    id: under
                    anchors.fill: parent
                    sourceItem: wall
                    sourceRect: Qt.rect(account.x + fieldBox.x + field.x,
                                        account.y + fieldBox.y, frost.width, frost.height)
                    visible: false
                }
                FastBlur { anchors.fill: parent; source: under; radius: 36 }
            }

            Rectangle {
                id: field
                width: parent.width
                height: parent.height
                radius: height / 2
                color: white(0x2e)
                border.width: 1
                border.color: white(failed ? 0x66 : 0x26)

                TextInput {
                    id: password
                    anchors.left: parent.left
                    anchors.leftMargin: 16 * u
                    anchors.right: go.left
                    anchors.rightMargin: 4
                    anchors.verticalCenter: parent.verticalCenter
                    echoMode: TextInput.Password
                    passwordCharacter: "•"
                    font.family: sans
                    font.pixelSize: 15 * u
                    color: "white"
                    selectionColor: white(0x55)
                    cursorVisible: activeFocus
                    clip: true
                    focus: true
                    enabled: !checking
                    Component.onCompleted: forceActiveFocus()
                    onAccepted: submit()
                    onTextChanged: if (text.length) failed = false
                    Keys.onEscapePressed: text = ""

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: password.text.length === 0
                        text: "Enter Password"
                        font: password.font
                        color: white(0x99)
                    }
                }

                // → appears once something is typed, like macOS
                Rectangle {
                    id: go
                    anchors.right: parent.right
                    anchors.rightMargin: 6 * u
                    anchors.verticalCenter: parent.verticalCenter
                    width: 26 * u
                    height: 26 * u
                    radius: width / 2
                    color: white(password.text.length ? 0x40 : 0)
                    Text {
                        anchors.centerIn: parent
                        text: checking ? "" : ""
                        font.family: icons
                        font.pixelSize: 18 * u
                        color: white(password.text.length || checking ? 0xee : 0)
                        RotationAnimation on rotation {
                            running: checking
                            from: 0; to: 360; duration: 900
                            loops: Animation.Infinite
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: submit()
                    }
                }
            }

            // macOS shakes the field: three quick swings that settle
            SequentialAnimation {
                id: shake
                NumberAnimation { target: field; property: "x"; to: -16 * u; duration: 50 }
                NumberAnimation { target: field; property: "x"; to: 14 * u; duration: 75 }
                NumberAnimation { target: field; property: "x"; to: -10 * u; duration: 75 }
                NumberAnimation { target: field; property: "x"; to: 6 * u; duration: 75 }
                NumberAnimation { target: field; property: "x"; to: 0; duration: 75 }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            height: 16 * u
            text: checking ? "Checking…" : failed ? "Wrong password" : ""
            font.family: sans
            font.pixelSize: 13 * u
            color: failed ? "#ff9f99" : white(0xb3)
        }
    }

    // ── very bottom: Sleep / Restart / Shut Down ──
    Timer { id: disarm; interval: 3000; onTriggered: armed = "" }

    Row {
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.height - 104 * u + (96 * u - height) / 2
        opacity: entry.content
        spacing: 44 * u

        Repeater {
            model: [
                { kind: "sleep",    glyph: "", label: "Sleep" },
                { kind: "restart",  glyph: "", label: "Restart" },
                { kind: "shutdown", glyph: "", label: "Shut Down" }
            ]
            delegate: Column {
                required property var modelData
                readonly property bool hot: armed === modelData.kind
                spacing: 8 * u

                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 46 * u
                    height: 46 * u
                    radius: width / 2
                    color: hot ? Qt.rgba(1, 0.27, 0.23, 0.4) : white(press.containsMouse ? 0x33 : 0x1f)
                    border.width: 1
                    border.color: hot ? "#ff8a83" : white(0x1a)
                    scale: press.pressed ? 0.92 : 1
                    Behavior on scale { NumberAnimation { duration: 90 } }
                    Behavior on color { ColorAnimation { duration: 120 } }
                    Text {
                        anchors.centerIn: parent
                        text: modelData.glyph
                        font.family: icons
                        font.pixelSize: 22 * u
                        color: "white"
                    }
                    MouseArea {
                        id: press
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (armed !== modelData.kind) {
                                armed = modelData.kind
                                disarm.restart()
                                return
                            }
                            armed = ""
                            if (modelData.kind === "sleep") sddm.suspend()
                            else if (modelData.kind === "restart") sddm.reboot()
                            else sddm.powerOff()
                        }
                    }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: hot ? "Click again" : modelData.label
                    font.family: sans
                    font.pixelSize: 12 * u
                    font.weight: Font.Medium
                    color: hot ? "#ffaaa5" : white(0xb3)
                }
            }
        }
    }

    // a click anywhere puts the caret back in the field
    MouseArea {
        anchors.fill: parent
        z: -1
        onClicked: password.forceActiveFocus()
    }
}
