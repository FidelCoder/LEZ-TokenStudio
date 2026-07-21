import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Logos.Controls
import Logos.Theme
import "../components"

ScrollView {
    id: root
    required property var controller
    readonly property bool busy: controller.backend ? controller.backend.busy : false
    property string trackedOperation: ""
    clip: true

    function runTracked(operation, label, arguments) {
        root.trackedOperation = operation
        if (!root.controller.runOperation(label, arguments))
            root.trackedOperation = ""
    }

    function publicKeyFromOutput(output) {
        const matches = String(output).match(/[0-9a-fA-F]{64}/g)
        return matches && matches.length > 0
            ? matches[matches.length - 1].toLowerCase()
            : ""
    }

    Connections {
        target: root.controller.backend
        function onOperationFinished(success, exitCode, output, error) {
            if (success && root.trackedOperation === "presenter-key") {
                const publicKey = root.publicKeyFromOutput(output)
                if (publicKey.length === 64)
                    presenterPublicKey.text = publicKey
            }
            root.trackedOperation = ""
        }
    }

    ColumnLayout {
        width: Math.max(root.availableWidth, 320)
        spacing: Theme.spacing.large

        SectionHeader {
            title: "Presenter"
            detail: "Create the local signing key committed by the private balance proof"
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width >= 760 ? 2 : 1
            columnSpacing: Theme.spacing.medium
            rowSpacing: Theme.spacing.small

            LabeledField { id: presenterKey; label: "Presenter key file"; text: "presenter.json" }
            LabeledField {
                id: presenterPublicKey
                label: "Presenter public key (hex)"
                placeholderText: "64 hexadecimal characters"
                validator: RegularExpressionValidator { regularExpression: /[0-9a-fA-F]{64}/ }
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Theme.spacing.medium

            Button {
                text: "Read public key"
                enabled: !root.busy && presenterKey.text.length > 0
                onClicked: root.runTracked("presenter-key", "Read presenter public key", [
                    "presenter", "public-key", "--key", presenterKey.text
                ])
            }
            Button {
                text: "Generate key"
                enabled: !root.busy && presenterKey.text.length > 0
                onClicked: root.runTracked("presenter-key", "Generate presenter key", [
                    "presenter", "generate", "--output", presenterKey.text
                ])
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.palette.borderSecondary
        }

        SectionHeader {
            title: "Balance proof"
            detail: "Generate a real Risc0 receipt from a witness or live wallet snapshot"
        }

        ButtonGroup { id: sourceGroup }
        Flow {
            Layout.fillWidth: true
            RadioButton {
                id: witnessMode
                text: "Witness JSON"
                checked: true
                ButtonGroup.group: sourceGroup
            }
            RadioButton {
                id: walletMode
                text: "Wallet + sequencer"
                ButtonGroup.group: sourceGroup
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width >= 760 ? 2 : 1
            columnSpacing: Theme.spacing.medium
            rowSpacing: Theme.spacing.small

            LabeledField { id: gateConfig; label: "Gate config"; text: "gate.json" }
            LabeledField { id: proofOutput; label: "Proof output"; text: "proof.json" }
            LabeledField {
                id: witnessInput
                label: "Private witness JSON"
                visible: witnessMode.checked
                text: "witness.json"
            }
            LabeledField {
                id: accountSnapshot
                label: "Private wallet snapshot"
                visible: walletMode.checked
                placeholderText: "account-snapshot.json"
            }
            LabeledField {
                id: sequencerUrl
                label: "Sequencer URL"
                visible: walletMode.checked
                placeholderText: "http://127.0.0.1:8080"
            }
            LabeledField {
                id: issuedAt
                label: "Issue time (Unix ms, optional)"
                placeholderText: "Uses current time"
            }
        }

        Flow {
            Layout.fillWidth: true
            Button {
                text: "Generate proof"
                enabled: !root.busy
                    && gateConfig.text.length > 0
                    && proofOutput.text.length > 0
                    && presenterPublicKey.text.length === 64
                    && ((witnessMode.checked && witnessInput.text.length > 0)
                        || (walletMode.checked && accountSnapshot.text.length > 0
                            && sequencerUrl.text.length > 0))
                onClicked: {
                    var args = [
                        "prove",
                        "--gate", gateConfig.text,
                        "--presenter-public-key-hex", presenterPublicKey.text,
                        "--output", proofOutput.text
                    ]
                    if (witnessMode.checked)
                        args.push("--input", witnessInput.text)
                    else
                        args.push("--account-snapshot", accountSnapshot.text,
                                  "--sequencer-url", sequencerUrl.text)
                    if (issuedAt.text.length > 0)
                        args.push("--issued-at-unix-ms", issuedAt.text)
                    root.controller.runOperation("Generate private balance proof", args)
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.palette.borderSecondary
        }

        SectionHeader {
            title: "Presentation"
            detail: "Sign a fresh verifier challenge for local, Messaging, or on-chain use"
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width >= 760 ? 2 : 1
            columnSpacing: Theme.spacing.medium
            rowSpacing: Theme.spacing.small

            LabeledField { id: challengeGate; label: "Gate config"; text: gateConfig.text }
            LabeledField { id: challengeOutput; label: "Challenge output"; text: "challenge.json" }
            LabeledField {
                id: challengeVerifier
                label: "Verifier ID override (optional)"
                placeholderText: "Use gate verifier by default"
            }
            LabeledField { id: challengeTtl; label: "Challenge lifetime (ms)"; text: "300000" }
            LabeledField { id: presentProof; label: "Proof"; text: proofOutput.text }
            LabeledField { id: presentChallenge; label: "Challenge"; text: challengeOutput.text }
            LabeledField { id: presentKey; label: "Presenter key"; text: presenterKey.text }
            LabeledField { id: envelopeOutput; label: "Envelope output"; text: "envelope.json" }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Theme.spacing.medium

            ComboBox {
                id: transport
                model: ["local", "logos-messaging", "on-chain"]
                Layout.preferredWidth: 180
            }
            Button {
                text: "Issue challenge"
                enabled: !root.busy && challengeGate.text.length > 0
                onClicked: {
                    var args = [
                        "challenge", "create",
                        "--gate", challengeGate.text,
                        "--ttl-ms", challengeTtl.text,
                        "--output", challengeOutput.text
                    ]
                    if (challengeVerifier.text.length > 0)
                        args.push("--verifier-id", challengeVerifier.text)
                    root.controller.runOperation("Issue verifier challenge", args)
                }
            }
            Button {
                text: "Create presentation"
                enabled: !root.busy
                    && presentProof.text.length > 0
                    && presentChallenge.text.length > 0
                    && presentKey.text.length > 0
                onClicked: root.controller.runOperation("Create proof presentation", [
                    "present",
                    "--proof", presentProof.text,
                    "--challenge", presentChallenge.text,
                    "--presenter-key", presentKey.text,
                    "--transport", transport.currentText,
                    "--output", envelopeOutput.text
                ])
            }
        }

        Item { Layout.preferredHeight: Theme.spacing.small }
    }
}
