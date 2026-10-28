import SwiftUI

/// The setup wizard's practice field: the task, a text field that types with ZhuyinType's own core and draws the
/// real candidate window under it, and 「完成」 once the task is done.  Everything typed stays in the field and in
/// practice data that is thrown away; the user's settings and data never change (docs/product/SETUP_WIZARD.md §8).
///
///     PracticeField(settings: SettingsSnapshot(model.sandbox), task: lesson.tasks[0],
///                   onProgress: { progress in … }, onComplete: { … })
///
/// A new `settings` value takes effect at once, even in the middle of a composition; a new task (another prompt)
/// starts the field over with `initialText`.
struct PracticeField: View {
    let settings: SettingsSnapshot
    let task: PracticeTask
    var initialText = ""
    var onProgress: ((PracticeProgress) -> Void)?
    var onComplete: (() -> Void)?
    /// The task line above the field; off when the wizard shows the task itself.
    var showsTask = true
    @StateObject private var model: PracticeFieldModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(settings: SettingsSnapshot, task: PracticeTask, initialText: String = "",
         onProgress: ((PracticeProgress) -> Void)? = nil, onComplete: (() -> Void)? = nil, showsTask: Bool = true) {
        self.settings = settings
        self.task = task
        self.initialText = initialText
        self.onProgress = onProgress
        self.onComplete = onComplete
        self.showsTask = showsTask
        _model = StateObject(wrappedValue: PracticeFieldModel(settings: settings, task: task, initialText: initialText))
    }

    /// A field around a model made beforehand (the UI previews drive it with keys).
    init(model: PracticeFieldModel) {
        settings = model.session.settings
        task = model.session.task
        initialText = model.session.initialText
        _model = StateObject(wrappedValue: model)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if showsTask, !task.prompt.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    HStack(alignment: .firstTextBaseline) {
                        Label(task.prompt, systemImage: "keyboard").font(.headline)
                        Spacer(minLength: 8)
                        if model.isComplete {
                            Label("完成", systemImage: "checkmark.circle.fill")
                                .font(.headline)
                                .foregroundStyle(Color(nsColor: .systemGreen))
                                .transition(reduceMotion ? .opacity : .scale(scale: 0.6).combined(with: .opacity))
                        }
                    }
                    if let hint = task.hint { Text(hint).font(.callout).foregroundStyle(.secondary) }
                }
                .accessibilityElement(children: .combine)
            }
            PracticeFieldRepresentable(model: model, settings: settings, task: task, initialText: initialText,
                                       onProgress: onProgress, onComplete: onComplete)
                .frame(height: model.height)
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .spring(response: 0.3, dampingFraction: 0.6),
                   value: model.isComplete)
        .onAppear { model.start() }
    }
}

/// Owns the practice session and its view for one field.
final class PracticeFieldModel: ObservableObject {
    let session: PracticeSession
    let view: PracticeFieldView
    @Published private(set) var isComplete = false
    @Published private(set) var height: CGFloat = 0
    @Published private(set) var isLoading = true
    var onProgress: ((PracticeProgress) -> Void)?
    var onComplete: (() -> Void)?
    private var started = false

    init(settings: SettingsSnapshot, task: PracticeTask, initialText: String = "") {
        session = PracticeSession(settings: settings, task: task, initialText: initialText)
        view = PracticeFieldView(session: session)
        height = view.intrinsicContentSize.height
        view.textView.placeholder = "正在準備…"
        view.textView.setAccessibilityLabel(task.prompt.isEmpty ? "練習" : task.prompt)
        session.onProgress = { [weak self] progress in
            guard let self else { return }
            if self.isComplete != progress.isComplete { self.isComplete = progress.isComplete }
            self.view.textView.isDone = progress.isComplete
            self.onProgress?(progress)
        }
        session.onComplete = { [weak self] in self?.onComplete?() }
    }

    /// Starts with the given practice data instead of loading it (self-tests, previews).
    convenience init(settings: SettingsSnapshot, task: PracticeTask, initialText: String = "", data: PracticeUserData) {
        self.init(settings: settings, task: task, initialText: initialText)
        attach(data)
    }

    var progress: PracticeProgress { session.progress }

    /// Loads (or shares) the practice data once the field is shown.
    func start() {
        guard !started else { return }
        started = true
        guard !session.isReady else { return }
        PracticeUserData.shared { [weak self] data in
            guard let self else { return }
            if let data {
                self.attach(data)
            } else {
                self.view.textView.placeholder = "無法載入詞庫"
            }
        }
    }

    private func attach(_ data: PracticeUserData) {
        started = true
        session.attach(data)
        isLoading = false
        view.textView.placeholder = "在這裡試打"
    }

    /// The wizard changed the draft settings or the task.
    func update(settings: SettingsSnapshot, task: PracticeTask, initialText: String) {
        if settings != session.settings {
            session.apply(settings)
            view.settingsChanged()
            height = view.intrinsicContentSize.height
        }
        if task.id != session.task.id || initialText != session.initialText {
            session.setTask(task, initialText: initialText)
            view.textView.setAccessibilityLabel(task.prompt.isEmpty ? "練習" : task.prompt)
        }
    }
}

private struct PracticeFieldRepresentable: NSViewRepresentable {
    let model: PracticeFieldModel
    let settings: SettingsSnapshot
    let task: PracticeTask
    let initialText: String
    let onProgress: ((PracticeProgress) -> Void)?
    let onComplete: (() -> Void)?

    func makeNSView(context: Context) -> PracticeFieldView { model.view }

    func updateNSView(_ view: PracticeFieldView, context: Context) {
        model.onProgress = onProgress
        model.onComplete = onComplete
        DispatchQueue.main.async { model.update(settings: settings, task: task, initialText: initialText) }
    }
}
