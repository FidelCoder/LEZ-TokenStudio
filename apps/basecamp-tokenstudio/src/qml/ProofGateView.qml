import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Logos.Controls
import Logos.Theme
import "pages"

Rectangle {
    id: root
    color: Theme.palette.background

    readonly property var backend: logos.module("tokenstudio_proofgate_ui")
    property bool ready: false
    property bool lastSuccess: true

    function runOperation(label, arguments) {
        if (!root.ready || !root.backend || root.backend.busy)
            return false
        return root.backend.start(label, arguments)
    }

    Connections {
        target: logos
        function onViewModuleReadyChanged(moduleName, isReady) {
            if (moduleName === "tokenstudio_proofgate_ui")
                root.ready = isReady && root.backend !== null
        }
    }

    Connections {
        target: root.backend
        function onOperationFinished(success, exitCode, output, error) {
            root.lastSuccess = success
        }
    }

    Component.onCompleted: {
        root.ready = root.backend !== null
            && logos.isViewModuleReady("tokenstudio_proofgate_ui")
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Theme.spacing.large
        spacing: Theme.spacing.medium

        RowLayout {
            Layout.fillWidth: true
            spacing: Theme.spacing.medium

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                LogosText {
                    text: "TokenStudio ProofGate"
                    color: Theme.palette.text
                    font.pixelSize: Theme.typography.titleText
                    font.weight: Theme.typography.weightBold
                }
                LogosText {
                    text: root.backend && root.backend.busy
                        ? root.backend.activeOperation
                        : "Private token access"
                    color: Theme.palette.textSecondary
                    font.pixelSize: Theme.typography.secondaryText
                }
            }

            BusyIndicator {
                running: root.backend && root.backend.busy
                visible: running
                Layout.preferredWidth: 28
                Layout.preferredHeight: 28
            }

            Button {
                text: "Cancel"
                visible: root.backend && root.backend.busy
                onClicked: root.backend.cancel()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Theme.spacing.small

            TextField {
                id: binaryField
                Layout.fillWidth: true
                text: root.backend ? root.backend.proofgateBinary : "proofgate"
                placeholderText: "ProofGate binary"
                selectByMouse: true
            }
            Button {
                text: "Use binary"
                enabled: root.ready && !(root.backend && root.backend.busy)
                onClicked: root.backend.configureProofgateBinary(binaryField.text)
            }
        }

        TabBar {
            id: tabs
            Layout.fillWidth: true
            enabled: root.ready

            TabButton { text: "Gate" }
            TabButton { text: "Prove" }
            TabButton { text: "Verify" }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabs.currentIndex

            GatePage { controller: root }
            ProvePage { controller: root }
            VerifyPage { controller: root }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 142
            color: Theme.palette.backgroundSecondary
            border.color: root.backend && root.backend.lastError.length > 0
                ? Theme.palette.error
                : Theme.palette.borderSecondary
            border.width: 1
            radius: 4

            ScrollView {
                anchors.fill: parent
                anchors.margins: Theme.spacing.small

                TextArea {
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.WrapAnywhere
                    color: root.backend && root.backend.lastError.length > 0
                        ? Theme.palette.error
                        : Theme.palette.text
                    text: {
                        if (!root.ready)
                            return "Connecting to ProofGate backend"
                        if (root.backend.lastError.length > 0)
                            return root.backend.lastError
                        if (root.backend.lastOutput.length > 0)
                            return root.backend.lastOutput
                        return "Ready"
                    }
                    background: null
                }
            }
        }
    }
}
