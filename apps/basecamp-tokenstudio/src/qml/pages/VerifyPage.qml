import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Logos.Controls
import Logos.Theme
import "../components"

Item {
    id: root
    required property var controller
    readonly property bool busy: controller.backend ? controller.backend.busy : false
    property string trackedOperation: ""

    function runTracked(operation, label, arguments) {
        root.trackedOperation = operation
        if (!root.controller.runOperation(label, arguments))
            root.trackedOperation = ""
    }

    function hexFromOutput(output) {
        const matches = String(output).match(/[0-9a-fA-F]{64}/g)
        return matches && matches.length > 0
            ? matches[matches.length - 1].toLowerCase()
            : ""
    }

    Connections {
        target: root.controller.backend
        function onOperationFinished(success, exitCode, output, error) {
            if (success && root.trackedOperation.length > 0) {
                const commitmentRoot = root.hexFromOutput(output)
                if (commitmentRoot.length === 64) {
                    if (root.trackedOperation === "messaging-root")
                        messageRoot.text = commitmentRoot
                    else if (root.trackedOperation === "chain-root")
                        chainRoot.text = commitmentRoot
                }
            }
            root.trackedOperation = ""
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Theme.spacing.medium

        TabBar {
            id: modes
            Layout.fillWidth: true
            TabButton { text: "Local" }
            TabButton { text: "Messaging" }
            TabButton { text: "On-chain" }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: modes.currentIndex

            ScrollView {
                id: localScroll
                clip: true

                ColumnLayout {
                    width: Math.max(localScroll.availableWidth, 320)
                    spacing: Theme.spacing.large

                    SectionHeader {
                        title: "Local verification"
                        detail: "Verify receipt, policy, freshness, presenter signature, and replay state"
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        columns: width >= 760 ? 2 : 1
                        columnSpacing: Theme.spacing.medium
                        rowSpacing: Theme.spacing.small

                        LabeledField { id: localGate; label: "Gate config"; text: "gate.json" }
                        LabeledField { id: localEnvelope; label: "Presentation envelope"; text: "envelope.json" }
                        LabeledField { id: localChallenge; label: "Issued challenge"; text: "challenge.json" }
                        LabeledField { id: localReplay; label: "Replay cache"; text: "replay-cache.json" }
                        LabeledField {
                            id: localVerifier
                            label: "Verifier ID override (optional)"
                            placeholderText: "Use gate verifier by default"
                        }
                        LabeledField {
                            id: localNow
                            label: "Verification time (Unix ms, optional)"
                            placeholderText: "Uses current time"
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        Button {
                            text: "Verify locally"
                            enabled: !root.busy
                                && localGate.text.length > 0
                                && localEnvelope.text.length > 0
                                && localChallenge.text.length > 0
                                && localReplay.text.length > 0
                            onClicked: {
                                var args = [
                                    "verify",
                                    "--gate", localGate.text,
                                    "--envelope", localEnvelope.text,
                                    "--challenge", localChallenge.text,
                                    "--replay-cache", localReplay.text
                                ]
                                if (localVerifier.text.length > 0)
                                    args.push("--verifier-id", localVerifier.text)
                                if (localNow.text.length > 0)
                                    args.push("--now-unix-ms", localNow.text)
                                root.controller.runOperation("Verify presentation", args)
                            }
                        }
                    }
                }
            }

            ScrollView {
                id: messagingScroll
                clip: true

                ColumnLayout {
                    width: Math.max(messagingScroll.availableWidth, 320)
                    spacing: Theme.spacing.large

                    SectionHeader {
                        title: "Encrypted transport"
                        detail: "Send a bounded proof transfer through the official Logos Chat module"
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        columns: width >= 760 ? 2 : 1
                        columnSpacing: Theme.spacing.medium
                        rowSpacing: Theme.spacing.small

                        LabeledField { id: sendLogoscore; label: "Logos Core CLI"; text: "logoscore" }
                        LabeledField { id: sendConfig; label: "Holder config directory" }
                        LabeledField { id: sendConversation; label: "Holder conversation ID" }
                        LabeledField { id: sendEnvelope; label: "Messaging envelope"; text: "envelope.json" }
                        LabeledField { id: verifierAddress; label: "Verifier sender address" }
                        LabeledField {
                            id: holderChallenge
                            label: "Received challenge output"
                            text: "holder-challenge.json"
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        Button {
                            text: "Receive challenge"
                            enabled: !root.busy
                                && sendConfig.text.length > 0
                                && sendConversation.text.length > 0
                                && verifierAddress.text.length > 0
                                && messageGate.text.length > 0
                                && targetGroup.text.length > 0
                                && senderAddress.text.length > 0
                            onClicked: root.controller.runOperation("Receive verifier challenge", [
                                "messaging", "receive-challenge",
                                "--logoscore-binary", sendLogoscore.text,
                                "--config-dir", sendConfig.text,
                                "--conversation-id", sendConversation.text,
                                "--expected-sender", verifierAddress.text,
                                "--gate", messageGate.text,
                                "--group-id", targetGroup.text,
                                "--member-address", senderAddress.text,
                                "--output", holderChallenge.text
                            ])
                        }
                        Button {
                            text: "Send proof"
                            enabled: !root.busy
                                && sendConfig.text.length > 0
                                && sendConversation.text.length > 0
                                && sendEnvelope.text.length > 0
                            onClicked: root.controller.runOperation("Send encrypted proof", [
                                "messaging", "send",
                                "--logoscore-binary", sendLogoscore.text,
                                "--config-dir", sendConfig.text,
                                "--conversation-id", sendConversation.text,
                                "--envelope", sendEnvelope.text
                            ])
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: Theme.palette.borderSecondary
                    }

                    SectionHeader {
                        title: "Verify and admit"
                        detail: "Pin the encrypted sender and add the proven member to one GroupV2 conversation"
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        columns: width >= 760 ? 2 : 1
                        columnSpacing: Theme.spacing.medium
                        rowSpacing: Theme.spacing.small

                        LabeledField { id: receiveLogoscore; label: "Logos Core CLI"; text: sendLogoscore.text }
                        LabeledField { id: receiveConfig; label: "Verifier config directory" }
                        LabeledField { id: receiveConversation; label: "Verifier conversation ID" }
                        LabeledField { id: transferId; label: "Transfer ID (optional)" }
                        LabeledField { id: senderAddress; label: "Member / sender address" }
                        LabeledField { id: targetGroup; label: "Target group ID" }
                        LabeledField { id: messageGate; label: "Gate config"; text: "gate.json" }
                        LabeledField {
                            id: messageSequencerUrl
                            label: "Sequencer URL"
                            text: "http://127.0.0.1:3040"
                        }
                        LabeledField {
                            id: messageRoot
                            label: "Trusted commitment root (hex)"
                            placeholderText: "64 hexadecimal characters"
                            validator: RegularExpressionValidator { regularExpression: /[0-9a-fA-F]{64}/ }
                        }
                        LabeledField {
                            id: admissionChallenge
                            label: "Admission challenge output"
                            text: "admission-challenge.json"
                        }
                        LabeledField { id: messageReplay; label: "Replay cache"; text: "chat-replay.json" }
                        LabeledField {
                            id: receivedEnvelope
                            label: "Verified envelope output"
                            text: "verified-envelope.json"
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        Button {
                            text: "Read sequencer root"
                            enabled: !root.busy && messageSequencerUrl.text.length > 0
                            onClicked: root.runTracked("messaging-root", "Read trusted commitment root", [
                                "sequencer-root",
                                "--sequencer-url", messageSequencerUrl.text
                            ])
                        }
                        Button {
                            text: "Issue admission challenge"
                            enabled: !root.busy
                                && senderAddress.text.length > 0
                                && targetGroup.text.length > 0
                                && messageRoot.text.length === 64
                            onClicked: root.controller.runOperation("Issue admission challenge", [
                                "messaging", "admission-challenge",
                                "--gate", messageGate.text,
                                "--commitment-root-hex", messageRoot.text,
                                "--group-id", targetGroup.text,
                                "--member-address", senderAddress.text,
                                "--output", admissionChallenge.text
                            ])
                        }
                        Button {
                            text: "Send challenge"
                            enabled: !root.busy
                                && receiveConfig.text.length > 0
                                && receiveConversation.text.length > 0
                                && admissionChallenge.text.length > 0
                            onClicked: root.controller.runOperation("Send verifier challenge", [
                                "messaging", "send-challenge",
                                "--logoscore-binary", receiveLogoscore.text,
                                "--config-dir", receiveConfig.text,
                                "--conversation-id", receiveConversation.text,
                                "--challenge", admissionChallenge.text
                            ])
                        }
                        Button {
                            text: "Verify and admit"
                            enabled: !root.busy
                                && receiveConfig.text.length > 0
                                && receiveConversation.text.length > 0
                                && senderAddress.text.length > 0
                                && targetGroup.text.length > 0
                                && messageGate.text.length > 0
                            onClicked: {
                                var args = [
                                    "messaging", "receive-verify",
                                    "--logoscore-binary", receiveLogoscore.text,
                                    "--config-dir", receiveConfig.text,
                                    "--conversation-id", receiveConversation.text,
                                    "--expected-sender", senderAddress.text,
                                    "--gate", messageGate.text,
                                    "--challenge", admissionChallenge.text,
                                    "--replay-cache", messageReplay.text,
                                    "--output", receivedEnvelope.text,
                                    "--admit-group-id", targetGroup.text,
                                    "--admit-address", senderAddress.text
                                ]
                                if (transferId.text.length > 0)
                                    args.push("--transfer-id", transferId.text)
                                root.controller.runOperation("Verify proof and admit member", args)
                            }
                        }
                    }
                }
            }

            ScrollView {
                id: onChainScroll
                clip: true

                ColumnLayout {
                    width: Math.max(onChainScroll.availableWidth, 320)
                    spacing: Theme.spacing.large

                    SectionHeader {
                        title: "LEZ access badge"
                        detail: "Deploy, initialize, prove, and submit through a LEZ sequencer"
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        columns: width >= 760 ? 2 : 1
                        columnSpacing: Theme.spacing.medium
                        rowSpacing: Theme.spacing.small

                        LabeledField { id: chainGate; label: "Gate config"; text: "gate.json" }
                        LabeledField {
                            id: sequencerUrl
                            label: "Sequencer URL"
                            text: "http://127.0.0.1:3040"
                        }
                        LabeledField { id: chainState; label: "On-chain gate state"; text: "gate-state.json" }
                        LabeledField { id: gateAccountKey; label: "Gate account key"; text: "gate-account.json" }
                        LabeledField {
                            id: chainNonce
                            label: "Initial challenge nonce (hex)"
                            placeholderText: "64 hexadecimal characters"
                        }
                        LabeledField { id: chainProof; label: "Balance proof"; text: "proof.json" }
                        LabeledField {
                            id: chainRoot
                            label: "Authorized commitment root (hex)"
                            placeholderText: "64 hexadecimal characters"
                            validator: RegularExpressionValidator { regularExpression: /[0-9a-fA-F]{64}/ }
                        }
                        LabeledField { id: chainKey; label: "Presenter key"; text: "presenter.json" }
                        LabeledField {
                            id: claimAccount
                            label: "Badge account ID (hex)"
                            placeholderText: "64 hexadecimal characters"
                        }
                        LabeledField { id: badgeAccountKey; label: "Badge account key"; text: "badge-account.json" }
                        LabeledField { id: chainClaim; label: "Claim output"; text: "claim.json" }
                        LabeledField {
                            id: gateAccount
                            label: "Gate account ID (hex)"
                            placeholderText: "64 hexadecimal characters"
                        }
                        LabeledField { id: lezProof; label: "LEZ proof output"; text: "lez-proof.bin" }
                        LabeledField { id: badgeOutput; label: "Access badge output"; text: "access-badge.json" }
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacing.medium
                        Button {
                            text: "Generate gate account"
                            enabled: !root.busy && gateAccountKey.text.length > 0
                            onClicked: root.controller.runOperation("Generate LEZ gate account", [
                                "on-chain", "account-generate",
                                "--output", gateAccountKey.text
                            ])
                        }
                        Button {
                            text: "Read sequencer root"
                            enabled: !root.busy && sequencerUrl.text.length > 0
                            onClicked: root.runTracked("chain-root", "Read authorized commitment root", [
                                "sequencer-root",
                                "--sequencer-url", sequencerUrl.text
                            ])
                        }
                        Button {
                            text: "Deploy program"
                            enabled: !root.busy && sequencerUrl.text.length > 0
                            onClicked: root.controller.runOperation("Deploy balance gate", [
                                "on-chain", "deploy",
                                "--sequencer-url", sequencerUrl.text
                            ])
                        }
                        Button {
                            text: "Initialize state"
                            enabled: !root.busy
                                && chainNonce.text.length === 64
                                && chainRoot.text.length === 64
                                && chainGate.text.length > 0
                            onClicked: root.controller.runOperation("Initialize on-chain gate", [
                                "on-chain", "init",
                                "--gate", chainGate.text,
                                "--commitment-root-hex", chainRoot.text,
                                "--challenge-nonce-hex", chainNonce.text,
                                "--output", chainState.text
                            ])
                        }
                        Button {
                            text: "Submit initialization"
                            enabled: !root.busy
                                && sequencerUrl.text.length > 0
                                && gateAccountKey.text.length > 0
                                && chainState.text.length > 0
                            onClicked: root.controller.runOperation("Submit gate initialization", [
                                "on-chain", "initialize-submit",
                                "--sequencer-url", sequencerUrl.text,
                                "--state", chainState.text,
                                "--gate-key", gateAccountKey.text
                            ])
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacing.medium
                        Button {
                            text: "Fetch gate state"
                            enabled: !root.busy
                                && sequencerUrl.text.length > 0
                                && gateAccount.text.length === 64
                            onClicked: root.controller.runOperation("Fetch current gate state", [
                                "on-chain", "fetch-state",
                                "--sequencer-url", sequencerUrl.text,
                                "--gate-account-id-hex", gateAccount.text,
                                "--output", chainState.text
                            ])
                        }
                        Button {
                            text: "Generate badge account"
                            enabled: !root.busy && badgeAccountKey.text.length > 0
                            onClicked: root.controller.runOperation("Generate LEZ badge account", [
                                "on-chain", "account-generate",
                                "--output", badgeAccountKey.text
                            ])
                        }
                        Button {
                            text: "Sign claim"
                            enabled: !root.busy
                                && claimAccount.text.length === 64
                            onClicked: root.controller.runOperation("Sign on-chain claim", [
                                "on-chain", "present",
                                "--proof", chainProof.text,
                                "--state", chainState.text,
                                "--claim-account-id-hex", claimAccount.text,
                                "--presenter-key", chainKey.text,
                                "--output", chainClaim.text
                            ])
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacing.medium
                        Button {
                            text: "Simulate"
                            enabled: !root.busy
                                && gateAccount.text.length === 64
                                && claimAccount.text.length === 64
                            onClicked: root.controller.runOperation("Execute SPEL gate", [
                                "on-chain", "simulate",
                                "--state", chainState.text,
                                "--claim", chainClaim.text,
                                "--gate-account-id-hex", gateAccount.text,
                                "--badge-account-id-hex", claimAccount.text
                            ])
                        }
                        Button {
                            text: "Compose LEZ proof"
                            enabled: !root.busy
                                && gateAccount.text.length === 64
                                && claimAccount.text.length === 64
                            onClicked: root.controller.runOperation("Compose LEZ private execution", [
                                "on-chain", "compose",
                                "--proof", chainProof.text,
                                "--state", chainState.text,
                                "--claim", chainClaim.text,
                                "--gate-account-id-hex", gateAccount.text,
                                "--badge-account-id-hex", claimAccount.text,
                                "--output-lez-proof", lezProof.text
                            ])
                        }
                        Button {
                            text: "Submit claim"
                            enabled: !root.busy
                                && sequencerUrl.text.length > 0
                                && gateAccount.text.length === 64
                                && badgeAccountKey.text.length > 0
                            onClicked: root.controller.runOperation("Compose and submit LEZ claim", [
                                "on-chain", "claim-submit",
                                "--sequencer-url", sequencerUrl.text,
                                "--proof", chainProof.text,
                                "--state", chainState.text,
                                "--claim", chainClaim.text,
                                "--gate-account-id-hex", gateAccount.text,
                                "--badge-key", badgeAccountKey.text,
                                "--output-lez-proof", lezProof.text
                            ])
                        }
                        Button {
                            text: "Fetch access badge"
                            enabled: !root.busy
                                && sequencerUrl.text.length > 0
                                && claimAccount.text.length === 64
                                && badgeOutput.text.length > 0
                            onClicked: root.controller.runOperation("Fetch LEZ access badge", [
                                "on-chain", "fetch-badge",
                                "--sequencer-url", sequencerUrl.text,
                                "--badge-account-id-hex", claimAccount.text,
                                "--output", badgeOutput.text
                            ])
                        }
                    }
                }
            }
        }
    }
}
