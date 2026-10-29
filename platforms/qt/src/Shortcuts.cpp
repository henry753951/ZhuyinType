#include "Shortcuts.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QJsonParseError>
#include <QSet>
#include <cmath>
#include <limits>

namespace {
// Only the keys the user changed, {command: [{key, modifiers}]}.  V1 stored every command's keys.
constexpr auto storageKey = "shortcuts/bindingsV2";
constexpr auto legacyKey = "shortcuts/bindingsV1";
#ifdef Q_OS_MACOS
// Qt swaps Command and Control on macOS: MetaModifier is the physical Control key.
constexpr int controlModifier = Qt::MetaModifier;
constexpr int appModifier = Qt::ControlModifier;
constexpr bool mac = true;
#else
constexpr int controlModifier = Qt::ControlModifier;
constexpr int appModifier = Qt::MetaModifier;
constexpr bool mac = false;
#endif
bool isPrintable(int key) { return key > Qt::Key_Space && key <= Qt::Key_AsciiTilde; }
bool isLetter(int key) { return key >= 'A' && key <= 'Z'; }
}

const QList<Shortcuts::Command> &Shortcuts::registry() {
    // The order of macOS ShortcutCommand.allCases (most used first); page is the Qt page of the command's 快捷鍵 row.
    static const QList<Command> result = {
        {"singleCharacterList", "只列一個字", "只列一個字，再按一次還原", "appearance", "feature.singleCharacterList", false, {{Qt::Key_Down, Qt::ShiftModifier}}},
        {"correctionsList", "列出修正的字", "列出按錯修正的字，再按一次還原", "choosing", "feature.correctionsList", false, {{Qt::Key_Up, Qt::ShiftModifier}}},
        {"typeZhuyin", "打出注音本身", "打出注音本身", "choosing", "feature.typeZhuyin", false, {{Qt::Key_QuoteLeft, 0}}},
        {"nextPage", "下一頁", "下一頁", "appearance", "feature.pageKeys", false, {{Qt::Key_Tab, 0}, {Qt::Key_PageDown, 0}, {Qt::Key_Down, Qt::AltModifier}}},
        {"previousPage", "上一頁", "上一頁", "appearance", "feature.pageKeys", false, {{Qt::Key_Tab, Qt::ShiftModifier}, {Qt::Key_PageUp, 0}, {Qt::Key_Up, Qt::AltModifier}}},
        {"selectLeft", "往左選取", "往左選取，再按 Return 加入自訂詞", "choosing", "feature.selectPhrase", false, {{Qt::Key_Left, Qt::ShiftModifier}}},
        {"selectRight", "往右選取", "往右選取，再按 Return 加入自訂詞", "choosing", "feature.selectPhrase", false, {{Qt::Key_Right, Qt::ShiftModifier}}},
        {"deleteWord", "刪掉一個詞", "刪掉一個詞", "typing", "feature.deleteWord", false, {{Qt::Key_Backspace, Qt::AltModifier}}},
        {"commitWithNewline", "送出並換行", "送出並換行", "typing", "", true, {{Qt::Key_Return, Qt::ShiftModifier}}},
        {"previousCharacter", "改前一個字", "清單開著時改前一個字", "appearance", "", true, {{Qt::Key_Left, Qt::AltModifier}}},
        {"nextCharacter", "改後一個字", "清單開著時改後一個字", "appearance", "", true, {{Qt::Key_Right, Qt::AltModifier}}}
    };
    return result;
}

const Shortcuts::Command *Shortcuts::definition(const QString &id) {
    for (const auto &command : registry()) if (command.id == id) return &command;
    return nullptr;
}

bool Shortcuts::normalize(Binding &binding) {
    constexpr int allowed = Qt::ShiftModifier | Qt::ControlModifier | Qt::AltModifier | Qt::MetaModifier | Qt::KeypadModifier;
    if (binding.modifiers & ~allowed) return false;
    binding.modifiers &= ~Qt::KeypadModifier;
    if (binding.key == Qt::Key_Backtab) {
        binding.key = Qt::Key_Tab;
        binding.modifiers |= Qt::ShiftModifier;
    }
    if (binding.key == Qt::Key_Enter) binding.key = Qt::Key_Return;
    if (binding.key >= 'a' && binding.key <= 'z') binding.key -= 'a' - 'A';
    // KeyEvent.key is the shifted symbol, unlike the macOS physical-key registry.
    const QString shifted = QStringLiteral("~!@#$%^&*()_+{}|:\"<>?");
    const QString unshifted = QStringLiteral("`1234567890-=[]\\;',./");
    const auto index = binding.key < 128 && binding.key >= 0 ? shifted.indexOf(QChar(binding.key)) : -1;
    if (index >= 0) {
        binding.key = unshifted.at(index).unicode();
        binding.modifiers |= Qt::ShiftModifier;
    }
    if (binding.key >= Qt::Key_Space && binding.key <= Qt::Key_AsciiTilde) return true;
    switch (binding.key) {
    case Qt::Key_Tab: case Qt::Key_Return: case Qt::Key_Escape: case Qt::Key_Backspace:
    case Qt::Key_Delete: case Qt::Key_Left: case Qt::Key_Right: case Qt::Key_Up: case Qt::Key_Down:
    case Qt::Key_Home: case Qt::Key_End: case Qt::Key_PageUp: case Qt::Key_PageDown: return true;
    default: return false;
    }
}

// MARK: - Labels

// The keycaps: modifiers in the macOS order ⌃⌥⇧⌘ (Ctrl Alt ⇧ Win/Super elsewhere), then the key.  On a Mac the keys a
// laptop types with fn read as fn plus the key they are printed on.
QStringList Shortcuts::keycaps(const Binding &b) {
    QStringList caps;
    if (b.modifiers & controlModifier) caps << (mac ? "⌃" : "Ctrl");
    if (b.modifiers & Qt::AltModifier) caps << (mac ? "⌥" : "Alt");
    if (b.modifiers & Qt::ShiftModifier) caps << "⇧";
#if defined(Q_OS_WIN)
    if (b.modifiers & appModifier) caps << "Win";
#else
    if (b.modifiers & appModifier) caps << (mac ? "⌘" : "Super");
#endif
    if (mac) {
        static const QMap<int, QString> fn = {{Qt::Key_PageUp, "↑"}, {Qt::Key_PageDown, "↓"}, {Qt::Key_Home, "←"},
                                             {Qt::Key_End, "→"}, {Qt::Key_Delete, "⌫"}};
        if (fn.contains(b.key)) return caps << "fn" << fn.value(b.key);
    }
    static const QMap<int, QString> names = {
        {Qt::Key_Space, "空白"}, {Qt::Key_Tab, "Tab"}, {Qt::Key_Return, "Return"}, {Qt::Key_Escape, "Esc"},
        {Qt::Key_Backspace, "⌫"}, {Qt::Key_Delete, mac ? "⌦" : "Del"}, {Qt::Key_Left, "←"}, {Qt::Key_Right, "→"},
        {Qt::Key_Up, "↑"}, {Qt::Key_Down, "↓"}, {Qt::Key_Home, "Home"}, {Qt::Key_End, "End"},
        {Qt::Key_PageUp, "PgUp"}, {Qt::Key_PageDown, "PgDn"}
    };
    caps << names.value(b.key, QString(QChar(b.key)).toUpper());
    return caps;
}

// One chip: "⇧↓", "⌃S", "fn↓" on a Mac; "Alt+↓", "Ctrl+⇧U", "⇧Tab", "PgDn" elsewhere.
QString Shortcuts::compact(const Binding &b) {
    const auto caps = keycaps(b);
    if (mac) return caps.size() == 1 ? caps.first() : caps.mid(0, caps.size() - 1).join(QString()) + caps.last();
    QString result;
    for (const auto &cap : caps.mid(0, caps.size() - 1)) result += cap == "⇧" ? cap : cap + "+";
    return result + caps.last();
}

// How the key is read aloud and in tooltips: 「fn ↓（Page Down）」 on a Mac, 「Ctrl+Shift+U」 elsewhere.
QString Shortcuts::spoken(const Binding &b) {
    if (mac) {
        static const QMap<int, QString> names = {{Qt::Key_PageUp, "Page Up"}, {Qt::Key_PageDown, "Page Down"},
            {Qt::Key_Home, "Home"}, {Qt::Key_End, "End"}, {Qt::Key_Delete, "向後刪除"}};
        return keycaps(b).join(' ') + (names.contains(b.key) ? "（" + names.value(b.key) + "）" : QString());
    }
    QStringList words;
    for (const auto &cap : keycaps(b)) words << (cap == "⇧" ? QStringLiteral("Shift") : cap);
    static const QMap<int, QString> names = {{Qt::Key_Backspace, "Backspace"}, {Qt::Key_Delete, "Delete"},
        {Qt::Key_PageUp, "Page Up"}, {Qt::Key_PageDown, "Page Down"}};
    if (names.contains(b.key)) words.last() = names.value(b.key);
    return words.join('+');
}

QString Shortcuts::modifierLabel(int modifiers) const {
    QStringList caps;
    if (modifiers & controlModifier) caps << (mac ? "⌃" : "Ctrl+");
    if (modifiers & Qt::AltModifier) caps << (mac ? "⌥" : "Alt+");
    if (modifiers & Qt::ShiftModifier) caps << "⇧";
#if defined(Q_OS_WIN)
    if (modifiers & appModifier) caps << "Win+";
#else
    if (modifiers & appModifier) caps << (mac ? "⌘" : "Super+");
#endif
    return caps.join(QString());
}

QVariantMap Shortcuts::describe(const Binding &binding) const {
    return {{"key", binding.key}, {"modifiers", binding.modifiers}, {"label", compact(binding)}, {"spoken", spoken(binding)}};
}

// MARK: - Rules

// Why `b` cannot be a key of `command` (macOS Shortcut.reservedReason(for:)), or "" when it can.  PgUp and PgDn page the
// list anyway, so they can be keys of 上一頁 and 下一頁.
QString Shortcuts::reserved(const Binding &b, const QString &command) const {
    const bool hasModifiers = b.modifiers != 0;
    if (!hasModifiers) {
        if ((b.key == Qt::Key_PageDown && command == "nextPage") || (b.key == Qt::Key_PageUp && command == "previousPage")) return {};
        switch (b.key) {
        case Qt::Key_PageUp: case Qt::Key_PageDown: return spoken(b) + " 用來翻頁";
        case Qt::Key_Home: return spoken(b) + " 移到組字的開頭";
        case Qt::Key_End: return spoken(b) + " 移到組字的結尾";
        case Qt::Key_Delete: return spoken(b) + " 刪除游標後的字";
        default: break;
        }
    }
#if defined(Q_OS_WIN)
    if (b.modifiers & appModifier) return QStringLiteral("Win 組合鍵留給系統");
#else
    if (b.modifiers & appModifier) return mac ? QStringLiteral("⌘ 組合鍵留給 App") : QStringLiteral("Super 組合鍵留給系統");
#endif
    const bool printable = isPrintable(b.key);
    const bool control = b.modifiers & controlModifier, alt = b.modifiers & Qt::AltModifier;
    if (!hasModifiers) {
        static const QSet<int> typing = {Qt::Key_Down, Qt::Key_Up, Qt::Key_Left, Qt::Key_Right, Qt::Key_Space,
            Qt::Key_Return, Qt::Key_Escape, Qt::Key_Backspace, Qt::Key_Delete, Qt::Key_Home, Qt::Key_End,
            Qt::Key_PageUp, Qt::Key_PageDown};
        if (typing.contains(b.key)) return QStringLiteral("這個鍵用在打字、選字或送出");
        if (printable && b.key != Qt::Key_QuoteLeft) return QStringLiteral("這個鍵用在打注音、聲調或標點");
    }
    if (printable && b.modifiers == Qt::ShiftModifier) return QStringLiteral("Shift 加這個鍵打中文標點或大寫字母");
    if (printable && alt && !control && !isLetter(b.key))
        return mac ? QStringLiteral("Option 加這個鍵打鍵上印的符號") : QStringLiteral("Alt 加這個鍵打鍵上印的符號");
    if (printable && b.modifiers == controlModifier && !isLetter(b.key) && features_.value("feature.controlPunctuation", true).toBool())
        return mac ? QStringLiteral("⌃ 加符號鍵打中文標點") : QStringLiteral("Ctrl 加符號鍵打中文標點");
    if (!mac && control && !alt && isLetter(b.key)) return QStringLiteral("Ctrl 加字母留給 App");
    return {};
}

// Why `binding` cannot be key `index` of `command`, naming the command that has it (ShortcutCommand.problem).
QString Shortcuts::problem(const Binding &binding, const QString &command, int index, const State &all) const {
    if (const auto reason = reserved(binding, command); !reason.isEmpty()) return reason;
    for (auto it = all.cbegin(); it != all.cend(); ++it)
        for (int position = 0; position < it.value().size(); ++position)
            if (it.value()[position] == binding && !(it.key() == command && position == index))
                return it.key() == command ? QStringLiteral("已是這個功能的另一個按鍵")
                                           : QStringLiteral("已用於「%1」").arg(definition(it.key())->label);
    return {};
}

// The user's keys, and the default keys for every other command, leaving out a default the user gave to another
// command (a command keeps all its defaults when every one is taken, and shows the conflict).
Shortcuts::State Shortcuts::withDefaults(const State &user) {
    State result = user;
    QList<Binding> taken;
    for (const auto &list : user) taken += list;
    for (const auto &command : registry()) {
        if (result.contains(command.id)) continue;
        QList<Binding> free;
        for (const auto &binding : command.defaults) if (!taken.contains(binding)) free << binding;
        result.insert(command.id, free.isEmpty() ? command.defaults : free);
    }
    return result;
}

bool Shortcuts::isEnabled(const QString &command) const {
    const auto *entry = definition(command);
    return entry && (entry->feature.isEmpty() || features_.value(entry->feature, true).toBool());
}

void Shortcuts::setFeatures(const QVariantMap &settings) {
    QVariantMap next;
    for (auto it = settings.cbegin(); it != settings.cend(); ++it)
        if (it.key().startsWith("feature.")) next.insert(it.key(), it.value());
    if (next == features_) return;
    features_ = next;
    emit changed();
}

// MARK: - Storage

Shortcuts::Shortcuts(QSettings &storage, QObject *parent) : QObject(parent), storage_(storage) {
    if (storage_.status() != QSettings::NoError) { fail(QStringLiteral("無法讀取按鍵設定檔；目前使用預設值。")); return; }
    const bool current = storage_.contains(storageKey);
    if (!current && !storage_.contains(legacyKey)) return;
    const auto bytes = storage_.value(current ? storageKey : legacyKey).toByteArray();
    if (bytes.size() > 65536) { fail(QStringLiteral("自訂按鍵資料過大；目前使用預設值，原始資料已保留。")); return; }
    QJsonParseError parse;
    const auto document = QJsonDocument::fromJson(bytes, &parse);
    if (parse.error != QJsonParseError::NoError || !document.isObject()) {
        fail(QStringLiteral("無法讀取自訂按鍵；目前使用預設值，原始資料已保留。"));
        return;
    }
    State loaded;
    const auto object = document.object();
    bool valid = true;
    QList<Binding> seen;
    for (auto it = object.begin(); it != object.end() && valid; ++it) {
        if (!definition(it.key()) || !it.value().isArray() || it.value().toArray().size() > maximumBindings()) { valid = false; break; }
        QList<Binding> list;
        for (const auto value : it.value().toArray()) {
            const auto row = value.toObject();
            const auto key = row.value("key"), modifiers = row.value("modifiers");
            const auto integer = [](const QJsonValue &v) {
                return v.isDouble() && std::isfinite(v.toDouble()) && std::floor(v.toDouble()) == v.toDouble()
                    && v.toDouble() >= 0 && v.toDouble() <= std::numeric_limits<int>::max();
            };
            if (!value.isObject() || row.size() != 2 || !integer(key) || !integer(modifiers)) { valid = false; break; }
            Binding binding{key.toInt(), modifiers.toInt()};
            auto canonical = binding;
            if (!normalize(canonical) || canonical != binding || seen.contains(binding)) { valid = false; break; }
            seen << binding;
            list << binding;
        }
        loaded.insert(it.key(), list);
    }
    if (valid && !current && loaded.size() != registry().size()) valid = false;
    if (!valid) { fail(QStringLiteral("無法讀取自訂按鍵；目前使用預設值，原始資料已保留。")); return; }
    for (auto it = loaded.cbegin(); it != loaded.cend(); ++it) {
        const auto *entry = definition(it.key());
        if (entry->required && it.value().isEmpty()) continue;
        // V1 stored every command: only those that differ from their defaults are the user's.
        if (current || it.value() != entry->defaults) user_.insert(it.key(), it.value());
    }
}

bool Shortcuts::fail(const QString &message) {
    error_ = message;
    emit errorChanged();
    return false;
}

// Stores `next` as the user's keys (Preferences.bindings' setter): a command that was on its defaults and still has
// what they resolved to stays on its defaults.
bool Shortcuts::store(const State &next) {
    const State resolved = bindings();
    State own;
    for (const auto &command : registry()) {
        const auto list = next.value(command.id);
        const bool keep = user_.contains(command.id) ? list != command.defaults
                                                     : list != resolved.value(command.id) && list != command.defaults;
        if (keep) own.insert(command.id, list);
    }
    QJsonObject object;
    for (auto it = own.cbegin(); it != own.cend(); ++it) {
        QJsonArray list;
        for (const auto &b : it.value()) list.append(QJsonObject{{"key", b.key}, {"modifiers", b.modifiers}});
        object.insert(it.key(), list);
    }
    const auto previous = storage_.value(storageKey);
    const bool existed = storage_.contains(storageKey);
    storage_.setValue(storageKey, QJsonDocument(object).toJson(QJsonDocument::Compact));
    storage_.remove(legacyKey);
    storage_.sync();
    if (storage_.status() != QSettings::NoError) {
        if (existed) storage_.setValue(storageKey, previous); else storage_.remove(storageKey);
        return fail(QStringLiteral("無法儲存按鍵，請檢查設定檔的寫入權限。"));
    }
    user_ = own;
    if (!error_.isEmpty()) { error_.clear(); emit errorChanged(); }
    emit changed();
    return true;
}

// MARK: - Reading

QVariantMap Shortcuts::values() const {
    QVariantMap result;
    const State all = bindings();
    for (auto it = all.cbegin(); it != all.cend(); ++it) {
        QVariantList list;
        for (const auto &binding : it.value()) list.append(describe(binding));
        result.insert(it.key(), list);
    }
    return result;
}

QVariantList Shortcuts::commands() const {
    QVariantList result;
    for (const auto &command : registry()) {
        QVariantList bindings;
        QStringList spokenDefaults;
        for (const auto &binding : command.defaults) {
            bindings.append(describe(binding));
            spokenDefaults << spoken(binding);
        }
        result.append(QVariantMap{{"id", command.id}, {"label", command.label}, {"title", command.title},
            {"page", command.page}, {"row", "keys-" + command.id}, {"feature", command.feature},
            {"required", command.required}, {"defaults", bindings}, {"defaultsSpoken", spokenDefaults.join(" 或 ")}});
    }
    return result;
}

QVariantMap Shortcuts::conflictsOf(const QString &command) const {
    QVariantMap result;
    if (!isEnabled(command)) return result;
    const State all = bindings();
    const auto list = all.value(command);
    for (int index = 0; index < list.size(); ++index)
        for (const auto &other : registry())
            if (other.id != command && isEnabled(other.id) && all.value(other.id).contains(list[index])) {
                result.insert(QString::number(index), other.id);
                break;
            }
    return result;
}

// Every conflicting pair, once each, for the 按鍵衝突 section.
QVariantList Shortcuts::conflicts() const {
    QVariantList result;
    QSet<QString> seen;
    const State all = bindings();
    for (const auto &command : registry()) {
        const auto found = conflictsOf(command.id);
        for (auto it = found.cbegin(); it != found.cend(); ++it) {
            const QString other = it.value().toString();
            const QString pair = command.id < other ? command.id + "|" + other : other + "|" + command.id;
            if (seen.contains(pair)) continue;
            seen.insert(pair);
            const auto binding = all.value(command.id).value(it.key().toInt());
            result.append(QVariantMap{{"command", command.id}, {"other", other}, {"commandLabel", command.label},
                {"otherLabel", definition(other)->label}, {"label", compact(binding)}});
        }
    }
    return result;
}

bool Shortcuts::anyCustomized() const { return !user_.isEmpty(); }

bool Shortcuts::isCustomized(const QString &command) const { return user_.contains(command); }

QString Shortcuts::activeLabel(const QString &command) const {
    if (!isEnabled(command)) return {};
    const auto list = bindings().value(command);
    return list.isEmpty() ? QString() : compact(list.first());
}

QString Shortcuts::resolve(int key, int modifiers) const {
    Binding needle{key, modifiers};
    if (!normalize(needle)) return {};
    const State all = bindings();
    // An enabled command wins over one whose feature is off (they may share a key).
    QString fallback;
    for (const auto &command : registry())
        if (all.value(command.id).contains(needle)) {
            if (isEnabled(command.id)) return command.id;
            if (fallback.isEmpty()) fallback = command.id;
        }
    return fallback;
}

// MARK: - Changing

bool Shortcuts::record(const QString &command, int key, int modifiers, int index) {
    const auto *entry = definition(command);
    if (!entry) return fail(QStringLiteral("找不到這個按鍵功能。"));
    Binding binding{key, modifiers};
    if (!normalize(binding)) return fail(QStringLiteral("這個鍵不能用"));
    State all = bindings();
    auto list = all.value(command);
    if (index == -1) index = int(list.size());
    if (index < 0 || index > list.size()) return fail(QStringLiteral("找不到要修改的按鍵。"));
    if (index == list.size() && list.size() >= maximumBindings())
        return fail(QStringLiteral("每個功能最多 %1 個按鍵").arg(maximumBindings()));
    if (const auto reason = problem(binding, command, index, all); !reason.isEmpty()) return fail(reason);
    if (index == list.size()) list.append(binding); else list[index] = binding;
    all.insert(command, list);
    return store(all);
}

bool Shortcuts::remove(const QString &command, int index) {
    const auto *entry = definition(command);
    State all = bindings();
    if (!entry || index < 0 || index >= all.value(command).size()) return fail(QStringLiteral("找不到要移除的按鍵。"));
    if (entry->required && all.value(command).size() == 1)
        return fail(QStringLiteral("「%1」至少要有一個按鍵").arg(entry->label));
    all[command].removeAt(index);
    return store(all);
}

bool Shortcuts::reset(const QString &command) {
    if (command.isEmpty()) {
        const State before = user_;
        user_.clear();
        if (!store(withDefaults({}))) { user_ = before; return false; }
        return true;
    }
    const auto *entry = definition(command);
    if (!entry) return fail(QStringLiteral("找不到這個按鍵功能。"));
    State all = bindings();
    all.insert(command, {});
    // The defaults another command now uses stay with it; the rest come back.
    QList<Binding> free;
    for (const auto &binding : entry->defaults) if (problem(binding, command, 0, all).isEmpty()) free << binding;
    if (free.isEmpty()) return fail(problem(entry->defaults.first(), command, 0, all));
    all.insert(command, free);
    return store(all);
}
