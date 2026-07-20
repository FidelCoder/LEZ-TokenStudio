#include "proofgate_ui_backend.h"

#include <QDir>
#include <QFileInfo>
#include <QProcessEnvironment>
#include <QSettings>
#include <QStandardPaths>
#include <QTimer>

namespace {

constexpr char kSettingsOrg[] = "Logos";
constexpr char kSettingsApp[] = "TokenStudioProofGate";
constexpr char kBinaryKey[] = "proofgateBinary";
constexpr int kStartTimeoutMs = 10'000;
constexpr int kCancelGraceMs = 3'000;

QString normalizedPath(const QString& value) {
    QString path = value.trimmed();
    if (path.startsWith(QLatin1Char('~'))) {
        path = QDir::homePath() + path.mid(1);
    }
    return path;
}

} // namespace

ProofGateUiBackend::ProofGateUiBackend()
    : ProofGateUiSimpleSource(), m_process(new QProcess(this)) {
    QSettings settings(kSettingsOrg, kSettingsApp);
    QString binary = settings.value(kBinaryKey).toString();
    if (binary.isEmpty()) {
        binary = QStandardPaths::findExecutable(QStringLiteral("proofgate"));
    }
    if (binary.isEmpty()) {
        binary = QStringLiteral("proofgate");
    }
    ProofGateUiSimpleSource::setProofgateBinary(binary);

    connect(m_process, &QProcess::readyReadStandardOutput, this, [this]() { drainOutput(); });
    connect(m_process, &QProcess::readyReadStandardError, this, [this]() { drainOutput(); });
    connect(
        m_process,
        qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
        this,
        [this](int code, QProcess::ExitStatus status) { finish(code, status); }
    );
    connect(m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart) {
            failToStart(m_process->errorString());
        }
    });
}

ProofGateUiBackend::~ProofGateUiBackend() {
    if (m_process->state() != QProcess::NotRunning) {
        m_process->kill();
        m_process->waitForFinished(1'000);
    }
}

void ProofGateUiBackend::onContextReady() {
}

bool ProofGateUiBackend::configureProofgateBinary(QString path) {
    if (busy()) {
        return false;
    }
    path = normalizedPath(path);
    if (path.isEmpty()) {
        return false;
    }
    const QString resolved = path.contains(QLatin1Char('/'))
        ? path
        : QStandardPaths::findExecutable(path);
    if (resolved.isEmpty() || !QFileInfo(resolved).isExecutable()) {
        ProofGateUiSimpleSource::setLastError(
            QStringLiteral("ProofGate binary is not executable: %1").arg(path)
        );
        return false;
    }
    ProofGateUiSimpleSource::setProofgateBinary(resolved);
    QSettings(kSettingsOrg, kSettingsApp).setValue(kBinaryKey, resolved);
    ProofGateUiSimpleSource::setLastError({});
    return true;
}

bool ProofGateUiBackend::start(QString operation, QStringList arguments) {
    if (busy() || arguments.isEmpty()) {
        return false;
    }
    const QString binary = proofgateBinary();
    const QString resolved = binary.contains(QLatin1Char('/'))
        ? binary
        : QStandardPaths::findExecutable(binary);
    if (resolved.isEmpty() || !QFileInfo(resolved).isExecutable()) {
        ProofGateUiSimpleSource::setLastError(
            QStringLiteral("ProofGate binary is not executable: %1").arg(binary)
        );
        return false;
    }

    m_stdout.clear();
    m_stderr.clear();
    ProofGateUiSimpleSource::setLastOutput({});
    ProofGateUiSimpleSource::setLastError({});
    ProofGateUiSimpleSource::setLastExitCode(0);
    ProofGateUiSimpleSource::setActiveOperation(operation.trimmed());
    ProofGateUiSimpleSource::setBusy(true);

    QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
    environment.insert(QStringLiteral("RISC0_DEV_MODE"), QStringLiteral("0"));
    m_process->setProcessEnvironment(environment);
    m_process->setProgram(resolved);
    m_process->setArguments(arguments);
    m_process->setProcessChannelMode(QProcess::SeparateChannels);
    m_process->start();
    if (!m_process->waitForStarted(kStartTimeoutMs)) {
        failToStart(m_process->errorString());
        return false;
    }
    return true;
}

bool ProofGateUiBackend::cancel() {
    if (!busy() || m_process->state() == QProcess::NotRunning) {
        return false;
    }
    m_process->terminate();
    QTimer::singleShot(kCancelGraceMs, m_process, [process = m_process]() {
        if (process->state() != QProcess::NotRunning) {
            process->kill();
        }
    });
    return true;
}

void ProofGateUiBackend::drainOutput() {
    m_stdout.append(m_process->readAllStandardOutput());
    m_stderr.append(m_process->readAllStandardError());
    ProofGateUiSimpleSource::setLastOutput(QString::fromUtf8(m_stdout));
    ProofGateUiSimpleSource::setLastError(QString::fromUtf8(m_stderr));
}

void ProofGateUiBackend::finish(int exitCode, QProcess::ExitStatus exitStatus) {
    drainOutput();
    const bool success = exitStatus == QProcess::NormalExit && exitCode == 0;
    const QString output = QString::fromUtf8(m_stdout).trimmed();
    QString error = QString::fromUtf8(m_stderr).trimmed();
    if (!success && error.isEmpty()) {
        error = exitStatus == QProcess::CrashExit
            ? QStringLiteral("ProofGate process crashed")
            : QStringLiteral("ProofGate exited with code %1").arg(exitCode);
    }
    ProofGateUiSimpleSource::setLastOutput(output);
    ProofGateUiSimpleSource::setLastError(error);
    ProofGateUiSimpleSource::setLastExitCode(exitCode);
    ProofGateUiSimpleSource::setBusy(false);
    ProofGateUiSimpleSource::setActiveOperation({});
    emit operationFinished(success, exitCode, output, error);
}

void ProofGateUiBackend::failToStart(const QString& message) {
    if (!busy()) {
        return;
    }
    ProofGateUiSimpleSource::setLastError(message);
    ProofGateUiSimpleSource::setLastExitCode(-1);
    ProofGateUiSimpleSource::setBusy(false);
    ProofGateUiSimpleSource::setActiveOperation({});
    emit operationFinished(false, -1, {}, message);
}
