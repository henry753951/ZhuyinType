#include "InputController.h"
#include <QDateTime>
#include <QHash>

namespace {
QPair<int, int> canonicalKey(int key, int modifiers) {
    modifiers &= ~int(Qt::KeypadModifier);
    if (key == Qt::Key_Backtab) { key = Qt::Key_Tab; modifiers |= Qt::ShiftModifier; }
    if (key == Qt::Key_Enter) key = Qt::Key_Return;
    return {key, modifiers};
}
double monotonicSeconds() {
    static QElapsedTimer clock;
    if (!clock.isValid()) clock.start();
    return double(clock.nsecsElapsed()) / 1e9;
}
bool startsComposition(const KeyInput &key) {
    return key.kind == KeyInput::Char || key.kind == KeyInput::ShiftedChar || key.kind == KeyInput::Printed
        || key.kind == KeyInput::ControlChar;
}
QString lastScalars(const QString &text, int count) {
    const auto all = text.toUcs4();
    if (all.size() <= count) return text;
    const auto tail = all.mid(all.size() - count);
    return QString::fromUcs4(reinterpret_cast<const char32_t *>(tail.constData()), tail.size());
}
QString dropLastScalar(const QString &text) {
    if (text.isEmpty()) return text;
    return text.left(text.size() - (text.size() >= 2 && text.at(text.size() - 1).isLowSurrogate() ? 2 : 1));
}
}

InputController::InputController(EngineSession &engine, Settings &settings, Shortcuts &shortcuts, QObject *parent)
    : QObject(parent), engine_(engine), settings_(settings), shortcuts_(shortcuts), commands_(shortcuts.commands()),
      english_(settings.values().value("startInEnglish").toBool()) {
    clock = monotonicSeconds;
    now = [] { return QDateTime::currentMSecsSinceEpoch(); };
    shortcuts_.setFeatures(settings_.values());
    connect(&settings_, &Settings::changed, this, [this] {
        shortcuts_.setFeatures(settings_.values());
        shiftTap_.reset();
        if (!secure_ && english() && engine_.ready() && engine_.composing()) passThrough();
        syncRecorder();
        emit modeChanged();
    });
    connect(&shortcuts_, &Shortcuts::changed, this, [this] { shiftTap_.reset(); });
    // What the engine sends while a key is handled: observed with that key, then added to the text before the
    // insertion point.
    connect(&engine_, &EngineSession::committedText, this, [this](const QString &text) {
        pendingCommit_ += text;
        if (secure_) return;
        recentText_ += text;
        remember();
    });
}

void InputController::setCorrectionLog(CorrectionLog *log, const QString &appVersion) {
    log_ = log;
    recorder_.reset();
    if (log_) {
        recorder_ = std::make_unique<CorrectionRecorder>([this](const QJsonObject &record) { if (log_) log_->append(record); });
        recorder_->appVersion = appVersion.isEmpty() ? QStringLiteral("dev") : appVersion;
        recorder_->clock = [this] { return clock(); };
    }
    syncRecorder();
}

void InputController::syncRecorder() {
    if (!recorder_) return;
    const auto values = settings_.values();
    recorder_->settings = correctionSettings(values);
    recorder_->selectionKeys = values.value("selectionKeys", "123456789").toString();
    recorder_->recordsContext = values.value("recordsCorrectionContext").toBool();
}

bool InputController::english() const {
    return english_ || (capsLock_ && settings_.values().value("feature.capsLockEnglish", true).toBool());
}

void InputController::setClient(const QString &app, std::optional<QString> before) {
    if (app != app_) {
        app_ = app;
        recentText_.clear();
        recentTime_ = std::numeric_limits<qint64>::min() / 2;
    }
    clientBefore_ = std::move(before);
}

// MARK: - The text before the insertion point (macOS ControllerCore.prepareContext / remember)

void InputController::prepareContext() {
    // Secure input: no text is read or remembered, and nothing is learned from this composition.
    if (secure_) { recentText_.clear(); return; }
    const qint64 time = now();
    if (time - recentTime_ > contextWindowMs) recentText_.clear();
    if (!settings_.values().value("usesPrecedingText", true).toBool()) return;
    if (clientBefore_) {
        recentText_ = lastScalars(*clientBefore_, contextRead);
        engine_.setPrecedingText(recentText_);
    } else if (recentText_.isEmpty() && appContext_.contains(app_)
               && time - appContext_.value(app_).second <= contextWindowMs) {
        recentText_ = appContext_.value(app_).first;
        // Without a reranker on this platform the remembered text is the composer's own preceding text.
        engine_.setPrecedingText(recentText_);
    } else if (!recentText_.isEmpty()) {
        engine_.setPrecedingText(recentText_);
    }
    recentTime_ = time;
}

void InputController::remember() {
    if (secure_) { recentText_.clear(); return; }
    recentText_ = lastScalars(recentText_, contextKept);
    recentTime_ = now();
    if (settings_.values().value("usesPrecedingText", true).toBool()) appContext_.insert(app_, {recentText_, recentTime_});
}

// MARK: - Keys

bool InputController::passThrough() {
    if (engine_.ready()) {
        if (engine_.composing()) {
            const auto before = Presentation::of(engine_);
            pendingCommit_.clear();
            const bool committed = engine_.commitAll();
            const QString commit = std::exchange(pendingCommit_, QString());
            observe({ObservedInput::Commit, {}, clock(), -1}, before, commit, true, std::nullopt);
            if (!committed) return false;
        }
        engine_.setPrecedingText({});
        if (recorder_) recorder_->breakContext();
    }
    return false;
}

bool InputController::engineKey(int key, int modifiers, double time) {
    const auto input = KeyInput::fromQt(key, modifiers);
    if (input && !engine_.composing() && startsComposition(*input)) prepareContext();
    const auto before = Presentation::of(engine_);
    pendingCommit_.clear();
    QElapsedTimer timer;
    timer.start();
    const bool handled = engine_.key(key, modifiers);
    const double latency = double(timer.nsecsElapsed()) / 1e6;
    const QString commit = std::exchange(pendingCommit_, QString());
    if (input) observe({ObservedInput::Key, *input, time, -1}, before, commit, handled, latency);
    if (!handled && input && !secure_) {
        // Keys the app receives extend or trim the text before the insertion point.
        switch (input->kind) {
        case KeyInput::Backspace: recentText_ = dropLastScalar(recentText_); remember(); break;
        case KeyInput::Space: recentText_ += ' '; remember(); break;
        case KeyInput::Enter: case KeyInput::ShiftEnter: recentText_ += '\n'; remember(); break;
        default: break;
        }
    }
    return handled;
}

bool InputController::feed(int key, int modifiers) {
    if (secure_ || english() || !engine_.ready()) return false;
    shiftTap_.reset();
    return engineKey(key, modifiers, clock());
}

void InputController::toggleCorrections() {
    if (secure_ || english() || !engine_.ready() || engine_.rows().isEmpty()) return;
    const auto before = Presentation::of(engine_);
    pendingCommit_.clear();
    engine_.key(Qt::Key_Up, Qt::ShiftModifier);
    observe({ObservedInput::ToggleCorrections, {}, clock(), -1}, before, std::exchange(pendingCommit_, QString()), true,
            std::nullopt);
}

bool InputController::isRemovedDefault(int key, int modifiers) const {
    const auto input = canonicalKey(key, modifiers);
    for (const auto &entry : commands_) {
        for (const auto &item : entry.toMap().value("defaults").toList()) {
            const auto binding = item.toMap();
            if (binding.value("key").toInt() == input.first && binding.value("modifiers").toInt() == input.second) return true;
        }
    }
    return false;
}

void InputController::shiftPressed(int modifiers, bool repeat, int side) {
    if (repeat || heldShifts_.contains(side)) return;
    if (side < UnknownShift || side > RightShift) { shiftTap_.reset(); return; }
    heldShifts_.insert(side);
    const QString allowedSide = settings_.values().value("shiftToggle", "both").toString();
    const bool allowed = allowedSide == "both" || (allowedSide == "left" && side == LeftShift)
        || (allowedSide == "right" && side == RightShift);
    shiftTap_.reset();
    if (!secure_ && allowed && heldShifts_.size() == 1 && !(modifiers & ~int(Qt::ShiftModifier))) {
        shiftTap_.emplace();
        shiftTap_->side = side;
        shiftTap_->timer.start();
    }
}

bool InputController::press(int key, int modifiers, bool repeat, int shiftSide) {
    const double time = clock();
    const bool wasOpen = !engine_.rows().isEmpty();
    const int oldSelection = engine_.selected();
    const auto command = shortcuts_.resolve(key, modifiers);
    const bool handled = pressImpl(key, modifiers, repeat, shiftSide, time);
    if (handled && wasOpen && !engine_.rows().isEmpty() && oldSelection == engine_.selected()) {
        const bool forward = command == "nextPage" || key == Qt::Key_PageDown
            || (key == Qt::Key_Down && modifiers == 0) || (key == Qt::Key_Tab && modifiers == 0);
        const bool backward = command == "previousPage" || key == Qt::Key_PageUp
            || (key == Qt::Key_Up && modifiers == 0) || key == Qt::Key_Backtab
            || (key == Qt::Key_Tab && modifiers == Qt::ShiftModifier);
        if (forward || backward) emit candidateBoundaryReached(forward ? 1 : -1);
    }
    return handled;
}

bool InputController::pressImpl(int key, int modifiers, bool repeat, int shiftSide, double time) {
    if (key == Qt::Key_Shift) {
        shiftPressed(modifiers, repeat, shiftSide);
        return false;
    }
    shiftTap_.reset();
    // Modifier-down is not a host shortcut yet; keep the composition for its following key.
    if (key == Qt::Key_Control || key == Qt::Key_Meta || key == Qt::Key_Alt || key == Qt::Key_AltGr
        || key == Qt::Key_Super_L || key == Qt::Key_Super_R) return false;
    if (key == Qt::Key_CapsLock) {
        if (!repeat) {
            const bool before = english();
            capsLock_ = !capsLock_;
            if (!secure_ && settings_.values().value("feature.capsLockEnglish", true).toBool()) passThrough();
            if (before != english()) {
                emit modeChanged();
                if (!secure_) emit modeSwitched(english());
            }
        }
        return false;
    }
    if (secure_ || !engine_.ready()) return false;
    if (english()) return passThrough();
    const auto command = shortcuts_.resolve(key, modifiers);
    if (!command.isEmpty()) {
        for (const auto &entry : commands_) {
            const auto definition = entry.toMap();
            if (definition.value("id") == command) {
                const auto feature = definition.value("feature").toString();
                if (!feature.isEmpty() && !settings_.values().value(feature, true).toBool()) return passThrough();
            }
        }
        return dispatch(command, key, modifiers, time);
    }
    if (isRemovedDefault(key, modifiers)) return passThrough();
    const bool handled = engineKey(key, modifiers, time);
    // EngineSession passes unsupported or disabled keys through without changing its composition.
    if (!handled && engine_.composing()) return passThrough();
    return handled;
}

bool InputController::dispatch(const QString &command, int originalKey, int originalModifiers, double time) {
    if (command == "previousCharacter" || command == "nextCharacter") {
        const auto arrow = command == "previousCharacter" ? Qt::Key_Left : Qt::Key_Right;
        if (engine_.rows().isEmpty() || settings_.values().value("candidateLayout") != "horizontal")
            return engineKey(arrow, 0, time);
        // Preserve the single-character list first, as macOS ControllerCore.neighbour does.
        const int reopen = engine_.single() ? Qt::Key_Down : engine_.corrections() ? Qt::Key_Up : Qt::Key_Down;
        const int modifiers = engine_.single() || engine_.corrections() ? int(Qt::ShiftModifier) : 0;
        engine_.key(Qt::Key_Escape);
        engine_.key(arrow);
        engine_.key(reopen, modifiers);
        return true;
    }
    static const QHash<QString, QPair<int, int>> inputs = {
        {"singleCharacterList", {Qt::Key_Down, Qt::ShiftModifier}},
        {"correctionsList", {Qt::Key_Up, Qt::ShiftModifier}},
        {"typeZhuyin", {Qt::Key_QuoteLeft, 0}}, {"nextPage", {Qt::Key_PageDown, 0}},
        {"previousPage", {Qt::Key_PageUp, 0}}, {"selectLeft", {Qt::Key_Left, Qt::ShiftModifier}},
        {"selectRight", {Qt::Key_Right, Qt::ShiftModifier}}, {"deleteWord", {Qt::Key_Backspace, Qt::AltModifier}},
        {"commitWithNewline", {Qt::Key_Return, Qt::ShiftModifier}}
    };
    const auto input = inputs.constFind(command);
    if (input == inputs.cend()) return passThrough();
    const bool handled = engineKey(input->first, input->second, time);
    if (!handled && command == "commitWithNewline"
        && canonicalKey(originalKey, originalModifiers) != qMakePair(int(Qt::Key_Return), int(Qt::ShiftModifier))) {
        emit insertTextRequested(QStringLiteral("\n"));
        return true;
    }
    if (!handled && engine_.composing()) return passThrough();
    return handled;
}

bool InputController::choose(int absoluteIndex) {
    if (secure_ || english() || !engine_.ready()) return false;
    shiftTap_.reset();
    const auto before = Presentation::of(engine_);
    pendingCommit_.clear();
    QElapsedTimer timer; timer.start();
    const bool handled = engine_.choose(absoluteIndex);
    const double elapsed = double(timer.nsecsElapsed()) / 1e6;
    if (handled) {
        observe({ObservedInput::Click, {}, clock(), absoluteIndex}, before, std::exchange(pendingCommit_, QString()), true,
                std::nullopt);
        emit candidateChosen(elapsed);
    }
    return handled;
}

void InputController::release(int key, int modifiers, int shiftSide) {
    if (key != Qt::Key_Shift) { shiftTap_.reset(); return; }
    if (!heldShifts_.remove(shiftSide)) return;
    const bool toggle = shiftTap_ && shiftTap_->side == shiftSide && heldShifts_.isEmpty()
        && !(modifiers & ~int(Qt::ShiftModifier)) && shiftTap_->timer.elapsed() <= 350
        && settings_.values().value("shiftSwitch", true).toBool();
    shiftTap_.reset();
    if (toggle && !secure_) {
        if (engine_.ready() && engine_.composing() && !passThrough() && engine_.composing()) return;
        if (engine_.ready()) engine_.setPrecedingText({});
        if (recorder_) recorder_->breakContext();
        english_ = !english_;
        emit modeChanged();
        emit modeSwitched(english());
    }
}

void InputController::setEnglish(bool english) {
    if (secure_ || english == english_ || (engine_.ready() && engine_.composing())) return;
    if (engine_.ready()) engine_.setPrecedingText({});
    if (recorder_) recorder_->breakContext();
    english_ = english;
    emit modeChanged();
    emit modeSwitched(InputController::english());
}

void InputController::clearShift() {
    heldShifts_.clear();
    shiftTap_.reset();
}

void InputController::discardSecureComposition() {
    if (!engine_.ready()) return;
    engine_.setLearningSuspended(true);
    engine_.reset();
    // The ABI ignores preceding-text updates while composing, so clear after reset.
    engine_.setPrecedingText({});
}

void InputController::setSecure(bool secure) {
    clearShift();
    if (secure || secure_) discardSecureComposition();
    secure_ = secure;
    if (secure) {
        recentText_.clear();
        usage_ = UsageState();
        if (recorder_) recorder_->observe({ObservedInput::Commit, {}, clock(), -1}, {}, {}, {}, true, true, [] { return QString(); });
    }
    if (engine_.ready()) engine_.setLearningSuspended(secure);
}

void InputController::deactivate() {
    clearShift();
    if (secure_) discardSecureComposition();
    else passThrough();
    clientBefore_.reset();
}

void InputController::synchronizeCapsLock(bool enabled) {
    if (capsLock_ == enabled) return;
    capsLock_ = enabled;
    if (!secure_ && english()) passThrough();
    emit modeChanged();
}

// MARK: - Observers

void InputController::observe(const ObservedInput &input, const Presentation &before, const QString &commit,
                              bool handled, std::optional<double> latencyMs) {
    const bool recording = recorder_ && settings_.values().value("recordsCorrections").toBool();
    const bool counting = settings_.values().value("recordsUsageStats").toBool();
    if (!recording && !counting) return;
    const auto after = Presentation::of(engine_);
    if (recording)
        recorder_->observe(input, before, after, commit, handled, secure_, [this] { return recentText_; });
    if (counting) observeUsage(input, before, after, commit, handled, latencyMs);
}

// macOS UsageStatsObserver, as events: OVERVIEW's store adds them up (IPC.md §使用統計).
void InputController::observeUsage(const ObservedInput &input, const Presentation &before, const Presentation &after,
                                   const QString &commit, bool handled, std::optional<double> latencyMs) {
    if (secure_) {
        if (usage_.lastKey) emit usageEvent("reset", {});
        usage_ = UsageState();
        return;
    }
    auto &u = usage_;
    const bool composing = !before.isEmpty() || !after.isEmpty();
    bool corrected = false;
    if (input.kind == ObservedInput::Key) {
        // Rhythm: the gap since the last key decides typing, thinking or away.
        const double time = input.time;
        if (u.lastKey) {
            const double gap = time - *u.lastKey;
            if (gap >= 0 && gap <= pauseThreshold) {
                emit usageEvent("interval", {{"ms", int(gap * 1000)}});
            } else if (gap > pauseThreshold && gap <= awayAfter) {
                emit usageEvent("pause", {{"ms", int(gap * 1000)}});
                u.burst = 0; u.cleanRun = 0;
            } else {
                u.burst = 0; u.cleanRun = 0;
            }
        }
        u.lastKey = time;
        // Every key the input method saw; `composing` when it was handled while composing (a counted key).
        emit usageEvent("key", {{"latencyMs", latencyMs.value_or(0)}, {"composing", handled && composing}});
    }
    // A pick: the list closed by a click, Return or a selection key.
    const QString selection = settings_.values().value("selectionKeys", "123456789").toString();
    const bool picked = input.kind == ObservedInput::Click
        || (input.kind == ObservedInput::Key && (input.key.is(KeyInput::Enter)
            || (input.key.kind == KeyInput::Char && selection.contains(QChar(input.key.character)))));
    if (before.candidates && !after.candidates && picked) {
        const auto &list = *before.candidates;
        const QString kind = list.tolerant == 1 ? "corrections" : list.single ? "single"
            : before.zhuyinHint.isEmpty() ? "punctuation" : "normal";
        emit usageEvent("pick", {{"list", kind}});
        corrected = true;
    }
    if (input.kind == ObservedInput::Key) {
        const auto &key = input.key;
        if (u.literalBefore && handled && (key.is(KeyInput::Char, u'`') || key.is(KeyInput::Escape)) && after.text == *u.literalBefore) {
            emit usageEvent("undoLiteral", {});
            u.literalBefore.reset();
        } else if (key.is(KeyInput::Char, u'`') && handled && !before.isEmpty() && after.text != before.text) {
            emit usageEvent("literalZhuyin", {});
            u.literalBefore = before.text;
            corrected = true;
        } else {
            u.literalBefore.reset();
            if (key.is(KeyInput::Backspace) || key.is(KeyInput::OptionBackspace)) {
                if (handled && !before.isEmpty() && !before.candidates) {
                    emit usageEvent("backspace", {});
                    corrected = true;
                } else if (!handled && before.isEmpty() && !u.deletedSinceCommit && clock() - u.lastCommit <= 30) {
                    emit usageEvent("deleteAfterCommit", {});
                    u.deletedSinceCommit = true;
                    corrected = true;
                }
            } else if (key.is(KeyInput::Escape)) {
                if (handled && !before.isEmpty() && !before.candidates && after.text != before.text) {
                    emit usageEvent("escape", {});
                    corrected = true;
                }
            }
        }
    } else if (input.kind == ObservedInput::Click) {
        u.literalBefore.reset();
    }
    if (corrected) { u.corrected = true; u.cleanRun = 0; }
    if (after.notice.startsWith(QStringLiteral("已加入自訂詞")) && after.notice != before.notice) emit usageEvent("phraseAdded", {});
    if (!commit.isEmpty()) {
        const int characters = int(commit.toUcs4().size());
        // This platform's engine has no reranker: no sentence is shown with a typing correction of its own, so
        // correctionShown and correctionCommitted are never sent.
        emit usageEvent("commit", {{"characters", characters}, {"compositionEnded", after.isEmpty()},
                                   {"clean", !u.corrected}, {"correctedShown", false}, {"correctedCommitted", false}});
        if (after.isEmpty()) {
            u.corrected = false;
            u.lastCommit = clock();
            u.deletedSinceCommit = false;
        }
    }
}
