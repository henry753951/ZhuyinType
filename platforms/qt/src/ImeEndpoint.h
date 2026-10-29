#pragma once
#include <QString>

// The same endpoint is used by the native adapters; access is restricted to this user.
QString imeEndpoint();
bool prepareImeEndpoint(const QString &endpoint, QString &error);
