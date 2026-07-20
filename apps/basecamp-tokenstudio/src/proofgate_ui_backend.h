#pragma once

#include <QByteArray>
#include <QProcess>
#include <QString>
#include <QStringList>

#include "logos_ui_plugin_context.h"
#include "rep_proofgate_ui_source.h"

class ProofGateUiBackend : public ProofGateUiSimpleSource, public LogosUiPluginContext {
public:
    ProofGateUiBackend();
    ~ProofGateUiBackend() override;

    void onContextReady() override;

    bool configureProofgateBinary(QString path) override;
    bool start(QString operation, QStringList arguments) override;
    bool cancel() override;

private:
    void finish(int exitCode, QProcess::ExitStatus exitStatus);
    void failToStart(const QString& message);
    void drainOutput();

    QProcess* m_process;
    QByteArray m_stdout;
    QByteArray m_stderr;
};
