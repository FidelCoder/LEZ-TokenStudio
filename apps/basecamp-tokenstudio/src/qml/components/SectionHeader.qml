import QtQuick
import QtQuick.Layouts
import Logos.Controls
import Logos.Theme

ColumnLayout {
    id: root
    property alias title: titleText.text
    property alias detail: detailText.text

    spacing: 3
    Layout.fillWidth: true

    LogosText {
        id: titleText
        color: Theme.palette.text
        font.pixelSize: Theme.typography.titleText
        font.weight: Theme.typography.weightBold
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }

    LogosText {
        id: detailText
        visible: text.length > 0
        color: Theme.palette.textSecondary
        font.pixelSize: Theme.typography.secondaryText
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
}
