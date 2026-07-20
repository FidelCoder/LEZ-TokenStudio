import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Logos.Controls
import Logos.Theme

ColumnLayout {
    id: root
    property alias label: fieldLabel.text
    property alias text: input.text
    property alias placeholderText: input.placeholderText
    property alias validator: input.validator
    property alias echoMode: input.echoMode
    property bool readOnly: false

    spacing: 5
    Layout.fillWidth: true

    LogosText {
        id: fieldLabel
        color: Theme.palette.textSecondary
        font.pixelSize: Theme.typography.secondaryText
    }

    TextField {
        id: input
        Layout.fillWidth: true
        Layout.preferredHeight: 40
        readOnly: root.readOnly
        selectByMouse: true
        color: Theme.palette.text
        placeholderTextColor: Theme.palette.textMuted
        background: Rectangle {
            color: root.readOnly
                ? Theme.palette.backgroundSecondary
                : Theme.palette.backgroundElevated
            border.color: input.activeFocus
                ? Theme.palette.borderInteractive
                : Theme.palette.borderSecondary
            border.width: 1
            radius: 4
        }
    }
}
