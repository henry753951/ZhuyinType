#include "DesktopServices.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <QNetworkReply>
#include <QTemporaryDir>
#include <QTimer>
#include <QtTest>
#include <cstring>

class TestReply final : public QNetworkReply {
public:
    TestReply(const QNetworkRequest &request, QByteArray body, int status, QObject *parent)
        : QNetworkReply(parent), body_(std::move(body)) {
        setRequest(request);
        setUrl(request.url());
        setAttribute(QNetworkRequest::HttpStatusCodeAttribute, status);
        open(QIODevice::ReadOnly);
        QTimer::singleShot(0, this, [this] {
            if (isFinished()) return;
            emit readyRead();
            if (isFinished()) return;
            setFinished(true);
            emit finished();
        });
    }
    void abort() override {
        if (isFinished()) return;
        setError(OperationCanceledError, "Canceled");
        setFinished(true);
        emit finished();
    }
    qint64 bytesAvailable() const override { return body_.size() - offset_ + QNetworkReply::bytesAvailable(); }
protected:
    qint64 readData(char *data, qint64 maxSize) override {
        const auto size = qMin(maxSize, qint64(body_.size() - offset_));
        if (size <= 0) return -1;
        std::memcpy(data, body_.constData() + offset_, size_t(size));
        offset_ += size;
        return size;
    }
private:
    QByteArray body_;
    qint64 offset_ = 0;
};
class TestNetwork final : public QNetworkAccessManager {
public:
    QByteArray body;
    int status = 200;
    int requests = 0;
    QNetworkRequest lastRequest;
protected:
    QNetworkReply *createRequest(Operation operation, const QNetworkRequest &request, QIODevice *) override {
        Q_ASSERT(operation == GetOperation);
        ++requests;
        lastRequest = request;
        return new TestReply(request, body, status, this);
    }
};
static QByteArray release(const QString &tag = "v0.4.8") {
    return QJsonDocument(QJsonObject{{"tag_name", tag}, {"draft", false}, {"prerelease", false},
        {"html_url", "https://github.com/henry753951/ZhuyinType/releases/tag/" + tag},
        {"assets", QJsonArray{QJsonObject{{"name", "ZhuyinType-0.4.8.pkg"}}}}}).toJson();
}

class DesktopServicesTest : public QObject {
    Q_OBJECT
private slots:
    void countsLikeMacOS() {
        QTemporaryDir directory;
        const auto path = directory.filePath("stats.json");
        QDateTime now(QDate(2026, 10, 25), QTime(9, 30)); // A Sunday.
        qint64 uptime = 0;
        {
            DesktopServices services(path, "0.4.7", nullptr, nullptr, [&] { return now; });
            services.setUptime([&] { return uptime; });
            // Off: nothing is counted.
            services.recordUsageEvent("commit", {{"characters", 5}});
            QCOMPARE(services.usage().value("characters").toLongLong(), 0);
            QVERIFY(services.setUsageEnabled(true));
            for (int i = 0; i < 4; ++i) {
                services.recordUsageEvent("key", {{"latencyMs", 2.5 + i}, {"composing", true}});
                uptime += 200;
            }
            services.recordUsageEvent("key", {{"latencyMs", 1}, {"composing", false}}); // Not composing: no key, no latency.
            services.recordUsageEvent("commit", {{"characters", 2}});
            uptime += 5000; // A thinking pause (over the 2 s threshold, under a minute).
            services.recordUsageEvent("key", {{"latencyMs", 40}});
            services.recordUsageEvent("pick", {{"list", "corrections"}});
            services.recordUsageEvent("backspace", {});
            services.recordUsageEvent("correctionShown", {});
            services.recordUsageEvent("commit", {{"characters", 3}});
            services.recordUsageEvent("phraseAdded", {});
            const auto usage = services.usage();
            QCOMPARE(usage.value("characters").toLongLong(), 5);
            QCOMPARE(usage.value("compositions").toLongLong(), 2);
            QCOMPARE(usage.value("cleanCompositions").toLongLong(), 1);
            QCOMPARE(usage.value("keys").toLongLong(), 5);
            QCOMPARE(usage.value("picksCorrections").toLongLong(), 1);
            QCOMPARE(usage.value("backspaces").toLongLong(), 1);
            QCOMPARE(usage.value("corrections").toLongLong(), 2);
            QCOMPARE(usage.value("correctedShown").toLongLong(), 1);
            QCOMPARE(usage.value("phrasesAdded").toLongLong(), 1);
            QCOMPARE(usage.value("typingMilliseconds").toLongLong(), 800);
            QCOMPARE(usage.value("thinkingMilliseconds").toLongLong(), 5000);
            QCOMPARE(usage.value("firstTimeRight").toDouble(), 0.5);
            const auto latency = usage.value("latency").toMap();
            QCOMPARE(latency.value("total").toLongLong(), 5);
            QCOMPARE(latency.value("maximum").toDouble(), 40.0);
            QVERIFY(latency.value("p50").toDouble() > 2 && latency.value("p50").toDouble() <= 5);
            const auto rhythm = usage.value("rhythm").toMap();
            QCOMPARE(rhythm.value("longestRun").toLongLong(), 3); // The pause ended the first burst.
            QCOMPARE(rhythm.value("longestCleanRun").toLongLong(), 3);
            QCOMPARE(rhythm.value("hours").toList().at(9).toLongLong(), 5); // Sunday 09:00.
            QCOMPARE(usage.value("streak").toInt(), 1);
            QCOMPARE(usage.value("bestDay").toMap().value("characters").toLongLong(), 5);
            // Written at most every 10 minutes: nothing yet, then on request.
            QCOMPARE(services.writes(), 0);
            QVERIFY(!QFile::exists(path));
            QVERIFY(services.saveIfNeeded());
            QCOMPARE(services.writes(), 1);
            QVERIFY(services.saveIfNeeded());
            QCOMPARE(services.writes(), 1);
            uptime += DesktopServices::SaveIntervalMs;
            services.recordUsageEvent("escape", {});
            QCOMPARE(services.writes(), 2);
        }
        // The macOS stats.json schema, read back.
        QFile file(path);
        QVERIFY(file.open(QIODevice::ReadOnly));
        const auto object = QJsonDocument::fromJson(file.readAll()).object();
        QCOMPARE(object.value("schema").toInt(), 2);
        QVERIFY(object.value("totals").toObject().contains("cleanCompositions"));
        QCOMPARE(object.value("latency").toObject().value("counts").toArray().size(), 22);
        QCOMPARE(object.value("rhythm").toObject().value("hours").toArray().size(), 168);
        QVERIFY(object.value("days").toObject().contains("2026-10-25"));
        DesktopServices again(path, "0.4.7", nullptr, nullptr, [&] { return now; });
        QCOMPARE(again.usage().value("characters").toLongLong(), 5);
        QCOMPARE(again.usage().value("escapes").toLongLong(), 1);
        QCOMPARE(again.usage().value("since").toString(), QString("2026-10-25"));
    }
    void periodsAndRetention() {
        QTemporaryDir directory;
        QDateTime now(QDate(2026, 1, 1), QTime(12, 0));
        DesktopServices services(directory.filePath("stats.json"), "0.4.7", nullptr, nullptr, [&] { return now; });
        QVERIFY(services.setUsageEnabled(true));
        services.recordUsageEvent("commit", {{"characters", 10}});
        now = now.addDays(29);
        services.recordUsageEvent("commit", {{"characters", 3}});
        QCOMPARE(services.usage().value("characters").toLongLong(), 13);
        QCOMPARE(services.usage().value("days").toList().size(), 30);
        now = now.addDays(1);
        QCOMPARE(services.usage().value("characters").toLongLong(), 3);
        QCOMPARE(services.usage().value("streak").toInt(), 1); // Yesterday counts while today has none.
        QVERIFY(services.setPeriod("all"));
        QCOMPARE(services.usage().value("characters").toLongLong(), 13);
        QVERIFY(!services.setPeriod("last-year"));
        for (int i = 0; i < 70; ++i) { now = now.addDays(1); services.recordUsageEvent("commit", {{"characters", 1}}); }
        QCOMPARE(services.usage().value("characters").toLongLong(), 83);
        QCOMPARE(services.usage().value("streak").toInt(), 60);
        QVERIFY(services.setPeriod("month"));
        QCOMPARE(services.usage().value("characters").toLongLong(), 30);
        QVERIFY(services.saveIfNeeded());
        QFile file(directory.filePath("stats.json"));
        QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(QJsonDocument::fromJson(file.readAll()).object().value("days").toObject().size(), DesktopServices::KeptDays);
    }
    void callerMeasuredRhythm() {
        DesktopServices services(QString(), "0.4.7");
        qint64 uptime = 0;
        services.setUptime([&] { return uptime; });
        QVERIFY(services.setUsageEnabled(true));
        services.recordUsageEvent("pause", {{"ms", 3000}});
        services.recordUsageEvent("interval", {{"ms", 150}});
        uptime += 9000;
        services.recordUsageEvent("key", {{"latencyMs", 3}}); // Gaps are no longer measured here.
        QCOMPARE(services.usage().value("thinkingMilliseconds").toLongLong(), 3000);
        QCOMPARE(services.usage().value("typingMilliseconds").toLongLong(), 150);
        QVERIFY(services.usage().value("rhythm").toMap().value("pauseMedian").toDouble() > 2);
        services.recordUsageEvent("pause", {{"ms", 61000}}); // Away, not thinking.
        QCOMPARE(services.usage().value("thinkingMilliseconds").toLongLong(), 3000);
    }
    void corruptedFileIsKeptUntilCleared() {
        QTemporaryDir directory;
        const auto path = directory.filePath("stats.json");
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write("{broken");
        file.close();
        DesktopServices services(path, "0.4.7");
        QVERIFY(services.setUsageEnabled(true));
        QVERIFY(!services.usageError().isEmpty());
        QVERIFY(!services.usage().value("valid").toBool());
        services.recordUsageEvent("commit", {{"characters", 1}});
        QVERIFY(services.saveIfNeeded());
        QVERIFY(file.open(QIODevice::ReadOnly));
        QCOMPARE(file.readAll(), QByteArray("{broken"));
        file.close();
        QVERIFY(services.clearUsage());
        QVERIFY(services.usage().value("valid").toBool());
        QVERIFY(!QFile::exists(path));
        services.recordUsageEvent("commit", {{"characters", 1}});
        QCOMPARE(services.usage().value("characters").toLongLong(), 1);
    }
    void invalidEventsAreIgnored() {
        DesktopServices services(QString(), "0.4.7");
        QVERIFY(services.setUsageEnabled(true));
        services.recordUsageEvent("commit", {{"characters", -1}});
        services.recordUsageEvent("commit", {{"characters", 1000001}});
        services.recordUsageEvent("key", {{"latencyMs", -4}});
        services.recordUsageEvent("unknown", {});
        QCOMPARE(services.usage().value("characters").toLongLong(), 0);
        QCOMPARE(services.usage().value("latency").toMap().value("total").toLongLong(), 0);
        QVERIFY(services.usage().value("latency").toMap().value("p95").isNull());
    }
    void processMetrics() {
        DesktopServices services(QString(), "0.4.7");
        const auto metrics = services.processMetrics();
        QVERIFY(metrics.value("cpuSeconds").toDouble() >= 0);
        QVERIFY(QDateTime::fromString(metrics.value("launched").toString(), Qt::ISODate).isValid());
#if defined(Q_OS_LINUX) || defined(Q_OS_WIN) || defined(Q_OS_MACOS)
        QVERIFY(metrics.value("memoryBytes").toDouble() > 0);
        QVERIFY(metrics.value("peakMemoryBytes").toDouble() >= metrics.value("memoryBytes").toDouble() * 0.5);
#endif
        QVERIFY(!metrics.contains("energyJoules"));
    }
    void updatesAreManualAndDoNotOfferMacInstaller() {
        TestNetwork network;
        network.body = release();
        DesktopServices services(QString(), "0.4.7", nullptr, &network);
        QCOMPARE(network.requests, 0);
        services.checkForUpdates();
        QVERIFY(services.checkingUpdate());
        services.checkForUpdates();
        QCOMPARE(network.requests, 1);
        QTRY_VERIFY(!services.checkingUpdate());
        QVERIFY(services.updateError().isEmpty());
        QCOMPARE(services.update().value("newer").toBool(), true);
        QCOMPARE(services.update().value("version").toString(), QString("v0.4.8"));
        QVERIFY(services.update().value("artifactStatus").toString().contains("尚未確認"));
        QVERIFY(!services.update().contains("downloadUrl"));
        QVERIFY(!services.update().value("checkedAt").toString().isEmpty());
        QCOMPARE(network.lastRequest.url().toString(), QString("https://api.github.com/repos/henry753951/ZhuyinType/releases/latest"));
        network.body = release("v0.4.6");
        services.checkForUpdates();
        QTRY_VERIFY(!services.checkingUpdate());
        QCOMPARE(services.update().value("newer").toBool(), false);
    }
    void invalidUpdateResponses_data() {
        QTest::addColumn<QByteArray>("body");
        QTest::addColumn<int>("status");
        QTest::newRow("http") << release() << 403;
        QTest::newRow("redirect") << release() << 302;
        QTest::newRow("syntax") << QByteArray("{") << 200;
        QTest::newRow("oversize") << QByteArray(256 * 1024 + 1, 'x') << 200;
        QTest::newRow("prerelease-version") << release("v0.5.0-beta") << 200;
        auto object = QJsonDocument::fromJson(release()).object();
        object.insert("html_url", "https://evil.example/releases/tag/v0.4.8");
        QTest::newRow("foreign-url") << QJsonDocument(object).toJson() << 200;
        object = QJsonDocument::fromJson(release()).object();
        object.insert("draft", true);
        QTest::newRow("draft") << QJsonDocument(object).toJson() << 200;
        object.insert("draft", false);
        object.insert("assets", QJsonArray{QJsonObject{{"name", QString(241, 'x')}}});
        QTest::newRow("asset-name") << QJsonDocument(object).toJson() << 200;
    }
    void invalidUpdateResponses() {
        QFETCH(QByteArray, body);
        QFETCH(int, status);
        TestNetwork network;
        network.body = body;
        network.status = status;
        DesktopServices services(QString(), "0.4.7", nullptr, &network);
        services.checkForUpdates();
        QTRY_VERIFY(!services.checkingUpdate());
        QVERIFY(!services.updateError().isEmpty());
        QVERIFY(!services.update().contains("releaseUrl"));
        QCOMPARE(services.update().value("status").toString(), QString("檢查失敗"));
    }
};
QTEST_GUILESS_MAIN(DesktopServicesTest)
#include "DesktopServicesTest.moc"
