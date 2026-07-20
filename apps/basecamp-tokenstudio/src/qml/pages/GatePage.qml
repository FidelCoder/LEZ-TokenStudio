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
    clip: true

    function tokenArguments(subcommand) {
        var args = [
            "token", subcommand,
            "--name", tokenName.text,
            "--symbol", tokenSymbol.text,
            "--decimals", decimals.text,
            "--definition-account-id-hex", definitionId.text,
            "--token-program-owner-hex", tokenOwner.text,
            "--definition-account", definitionAccount.text,
            "--supply-account", supplyAccount.text,
            "--issuer-account", issuerAccount.text,
            "--total-supply", totalSupply.text,
            "--output", tokenOutput.text
        ]
        if (subcommand === "create") {
            args.push("--wallet-binary", walletBinary.text)
            if (dryRun.checked)
                args.push("--dry-run")
        }
        return args
    }

    ColumnLayout {
        width: Math.max(root.availableWidth, 320)
        spacing: Theme.spacing.large

        SectionHeader {
            title: "Token"
            detail: "Create, register, or mint an LEZ token"
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width >= 760 ? 2 : 1
            columnSpacing: Theme.spacing.medium
            rowSpacing: Theme.spacing.small

            LabeledField { id: tokenName; label: "Name"; placeholderText: "Founders Access" }
            LabeledField { id: tokenSymbol; label: "Symbol"; placeholderText: "FOUND" }
            LabeledField { id: decimals; label: "Decimals"; text: "0" }
            LabeledField { id: totalSupply; label: "Total supply"; placeholderText: "1000000" }
            LabeledField {
                id: definitionId
                label: "Definition account ID (hex)"
                placeholderText: "64 hexadecimal characters"
            }
            LabeledField {
                id: tokenOwner
                label: "Token program owner (hex)"
                placeholderText: "64 hexadecimal characters"
            }
            LabeledField { id: definitionAccount; label: "Definition account" }
            LabeledField { id: supplyAccount; label: "Supply account" }
            LabeledField { id: issuerAccount; label: "Issuer account" }
            LabeledField { id: tokenOutput; label: "Token config output"; text: "token.json" }
            LabeledField { id: walletBinary; label: "LEZ wallet binary"; text: "wallet" }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Theme.spacing.medium

            CheckBox {
                id: dryRun
                text: "Plan only"
                checked: true
            }
            Button {
                text: "Save existing"
                enabled: !root.busy
                    && tokenName.text.length > 0
                    && tokenSymbol.text.length > 0
                    && decimals.text.length > 0
                    && totalSupply.text.length > 0
                    && definitionId.text.length === 64
                    && tokenOwner.text.length === 64
                    && definitionAccount.text.length > 0
                    && supplyAccount.text.length > 0
                    && issuerAccount.text.length > 0
                onClicked: root.controller.runOperation(
                    "Save existing token", root.tokenArguments("select"))
            }
            Button {
                text: dryRun.checked ? "Preview creation" : "Create token"
                enabled: !root.busy
                    && tokenName.text.length > 0
                    && tokenSymbol.text.length > 0
                    && decimals.text.length > 0
                    && totalSupply.text.length > 0
                    && definitionId.text.length === 64
                    && tokenOwner.text.length === 64
                    && definitionAccount.text.length > 0
                    && supplyAccount.text.length > 0
                    && issuerAccount.text.length > 0
                    && walletBinary.text.length > 0
                onClicked: root.controller.runOperation(
                    dryRun.checked ? "Preview token creation" : "Create token",
                    root.tokenArguments("create"))
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.palette.borderSecondary
        }

        SectionHeader {
            title: "Mint"
            detail: "Issue an amount to a holder account"
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width >= 760 ? 2 : 1
            columnSpacing: Theme.spacing.medium
            rowSpacing: Theme.spacing.small

            LabeledField { id: mintToken; label: "Token config"; text: tokenOutput.text }
            LabeledField { id: mintHolder; label: "Holder account" }
            LabeledField { id: mintAmount; label: "Amount"; placeholderText: "100" }
            LabeledField { id: mintWallet; label: "LEZ wallet binary"; text: walletBinary.text }
        }

        Flow {
            Layout.fillWidth: true
            CheckBox { id: mintDryRun; text: "Plan only"; checked: true }
            Button {
                text: "Mint"
                enabled: !root.busy
                    && mintToken.text.length > 0
                    && mintHolder.text.length > 0
                    && mintAmount.text.length > 0
                onClicked: {
                    var args = [
                        "token", "mint",
                        "--token", mintToken.text,
                        "--holder", mintHolder.text,
                        "--amount", mintAmount.text,
                        "--wallet-binary", mintWallet.text
                    ]
                    if (mintDryRun.checked)
                        args.push("--dry-run")
                    root.controller.runOperation("Mint token", args)
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.palette.borderSecondary
        }

        SectionHeader {
            title: "Gate policy"
            detail: "Bind one token definition and exact threshold to an application context"
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width >= 760 ? 2 : 1
            columnSpacing: Theme.spacing.medium
            rowSpacing: Theme.spacing.small

            LabeledField { id: gateToken; label: "Token config"; text: tokenOutput.text }
            LabeledField { id: applicationId; label: "Application ID"; text: "tokenstudio" }
            LabeledField { id: gateId; label: "Gate ID"; placeholderText: "founders-chat" }
            LabeledField { id: threshold; label: "Minimum balance"; placeholderText: "100" }
            LabeledField {
                id: verifierId
                label: "Verifier ID"
                placeholderText: "logos-chat:founders"
            }
            LabeledField {
                id: expiry
                label: "Expiry (Unix ms, optional)"
                placeholderText: "1800000000000"
            }
            LabeledField { id: gateOutput; label: "Gate config output"; text: "gate.json" }
        }

        Flow {
            Layout.fillWidth: true
            Button {
                text: "Create gate"
                enabled: !root.busy
                    && gateToken.text.length > 0
                    && gateId.text.length > 0
                    && threshold.text.length > 0
                    && verifierId.text.length > 0
                onClicked: {
                    var args = [
                        "gate", "init",
                        "--token", gateToken.text,
                        "--application-id", applicationId.text,
                        "--gate-id", gateId.text,
                        "--threshold", threshold.text,
                        "--verifier-id", verifierId.text,
                        "--output", gateOutput.text
                    ]
                    if (expiry.text.length > 0)
                        args.push("--expires-at-unix-ms", expiry.text)
                    root.controller.runOperation("Create gate", args)
                }
            }
        }

        Item { Layout.preferredHeight: Theme.spacing.small }
    }
}
