import SwiftUI

// Small animated pictures for the 視窗與提示 page: each shows only the part one setting changes, in about 70 pt, in
// place of a second full preview.  They follow the settings live, loop slowly (still under Reduce Motion, and in the UI
// previews at `DemoPinning.vignettePhase`), and never take input.

/// The frame every vignette sits in: a strip of document with the text line and the bit of window it is about.
struct Vignette<Content: View>: View {
    var height: CGFloat = 70
    @ViewBuilder let content: () -> Content
    var body: some View {
        content()
            .frame(maxWidth: .infinity, minHeight: height, maxHeight: height)
            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color(nsColor: .textBackgroundColor)))
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(.quaternary, lineWidth: 1))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

/// A slow loop through `count` phases, one every `interval` seconds; still (at `rest`) under Reduce Motion and in previews.
struct VignetteLoop<Content: View>: View {
    let count: Int
    var interval: Double = 1.6
    var rest = 0
    @ViewBuilder let content: (Int) -> Content
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if let pinned = DemoPinning.vignettePhase {
            content(pinned % count)
        } else if reduceMotion {
            content(rest)
        } else {
            TimelineView(.periodic(from: .now, by: interval)) { context in
                let phase = Int(context.date.timeIntervalSinceReferenceDate / interval) % count
                content(phase).animation(.easeOut(duration: 0.25), value: phase)
            }
        }
    }
}

/// A few candidate rows, as the window draws them, small.
struct VignetteList: View {
    let words: [String]
    var annotations: [String?] = []
    var selected = 0
    var spacing: CGFloat = 0
    var rowHeight: CGFloat = 15
    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                HStack(spacing: 5) {
                    Text("\(index + 1)").font(.system(size: 8, weight: .medium)).monospacedDigit()
                        .foregroundStyle(index == selected ? AnyShapeStyle(Color.white.opacity(0.85)) : AnyShapeStyle(.secondary))
                    Text(word).font(.system(size: 11))
                        .foregroundStyle(index == selected ? AnyShapeStyle(Color.white) : AnyShapeStyle(.primary))
                    if let note = annotations.indices.contains(index) ? annotations[index] : nil {
                        Text(note).font(.system(size: 8))
                            .foregroundStyle(index == selected ? AnyShapeStyle(Color.white.opacity(0.8)) : AnyShapeStyle(.tertiary))
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 5)
                .frame(height: rowHeight)
                .background(RoundedRectangle(cornerRadius: 4, style: .continuous)
                    .fill(index == selected ? Color.accentColor : .clear))
            }
        }
        .padding(3)
        .frame(width: annotations.isEmpty ? 104 : 168)
        .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(.regularMaterial))
        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(.quaternary, lineWidth: 0.5))
        .shadow(color: .black.opacity(0.12), radius: 3, y: 1)
    }
}

/// The document line: committed text, the composition underlined, a caret.
private struct VignetteLine: View {
    var committed = "約了"
    var marked = "王曉明"
    var body: some View {
        HStack(spacing: 0) {
            Text(committed)
            Text(marked).underline()
            Rectangle().fill(.primary).frame(width: 1.2, height: 13)
        }
        .font(.system(size: 13))
    }
}

// MARK: - The vignettes

/// 修正清單 and 標出修正的讀音: ⇧↑ swaps the list to the corrected characters, the corrected reading marked in grey.
struct CorrectionsVignette: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        let key = CurrentKeys(preferences).text(.correctionsList)
        Vignette {
            VignetteLoop(count: 2, interval: 1.8, rest: 1) { phase in
                HStack(alignment: .top, spacing: 14) {
                    VStack(alignment: .leading, spacing: 6) {
                        VignetteLine()
                        if let key { KeyChip(text: key).opacity(phase == 1 ? 1 : 0.35) }
                    }
                    if phase == 0 {
                        VignetteList(words: ["曉明", "小明", "小名"]).transition(.opacity)
                    } else {
                        VignetteList(words: ["校名", "孝明", "曉明"],
                                 annotations: preferences.showsAnnotations ? ["ㄒㄧㄠˋ ㄇㄧㄥˊ", "ㄒㄧㄠˋ ㄇㄧㄥˊ", nil] : [])
                            .transition(.opacity)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 12)
                .padding(.top, 8)
            }
        }
    }
}

/// 位置: the window under or over the line, snapping to the chosen side.
struct PlacementVignette: View {
    @ObservedObject var preferences: Preferences
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        let above = preferences.candidatePlacement == .above
        Vignette(height: 96) {
            VStack(alignment: .leading, spacing: 4) {
                if above { VignetteList(words: ["曉明", "小明"]).transition(.move(edge: .bottom).combined(with: .opacity)) }
                VignetteLine()
                if !above { VignetteList(words: ["曉明", "小明"]).transition(.move(edge: .top).combined(with: .opacity)) }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .padding(.horizontal, 12)
            .animation(reduceMotion ? .easeInOut(duration: 0.2) : .spring(response: 0.35, dampingFraction: 0.8), value: above)
        }
    }
}

/// 每頁的候選字 and 間距: a slice of the list re-flowing.
struct DensityVignette: View {
    @ObservedObject var preferences: Preferences
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        let compact = preferences.candidateDensity == .compact
        let count = preferences.candidatesPerPage
        let words = Array(["曉明", "小明", "小名", "明", "名", "鳴", "銘", "冥", "茗"].prefix(count))
        Vignette(height: 76) {
            HStack(alignment: .top, spacing: 0) {
                Group {
                    if preferences.candidateLayout == .vertical {
                        VignetteList(words: Array(words.prefix(3)), spacing: compact ? 0 : 2, rowHeight: compact ? 13 : 16)
                    } else {
                        HStack(spacing: compact ? 4 : 10) {
                            ForEach(Array(words.prefix(6).enumerated()), id: \.offset) { index, word in
                                Text("\(index + 1) \(word)").font(.system(size: 11)).fixedSize()
                                    .foregroundStyle(index == 0 ? AnyShapeStyle(Color.white) : AnyShapeStyle(.primary))
                                    .padding(.horizontal, 4).padding(.vertical, 1)
                                    .background(RoundedRectangle(cornerRadius: 4).fill(index == 0 ? Color.accentColor : .clear))
                            }
                        }
                        .padding(compact ? 3 : 5)
                        .fixedSize()
                        .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(.regularMaterial))
                        .frame(maxWidth: 330, alignment: .leading)
                        .clipped()
                    }
                }
                Spacer(minLength: 0)
                Text("每頁 \(count) 個").font(.caption).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .padding(.top, 8)
            .frame(maxHeight: .infinity, alignment: .top)
            .animation(reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.85), value: compact)
            .animation(reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.85), value: count)
        }
    }
}

/// 動畫: turning a page, with the motion the setting gives (none when it is off).
struct MotionVignette: View {
    @ObservedObject var preferences: Preferences
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        let animates = preferences.panelAnimation
        Vignette {
            VignetteLoop(count: 2, interval: 1.4, rest: 0) { phase in
                HStack(alignment: .top, spacing: 14) {
                    VStack(alignment: .leading, spacing: 6) {
                        VignetteLine()
                        KeyChip(text: CurrentKeys(preferences).text(.nextPage) ?? "Tab")
                    }
                    ZStack(alignment: .top) {
                        let page = phase == 0 ? ["曉明", "小明", "小名"] : ["溟", "螟", "暝"]
                        VignetteList(words: page)
                            .id(phase)
                            .transition(animates && !reduceMotion
                                        ? .asymmetric(insertion: .offset(y: 8).combined(with: .opacity), removal: .offset(y: -8).combined(with: .opacity))
                                        : .identity)
                    }
                    .animation(animates ? .easeOut(duration: 0.14) : nil, value: phase)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 12)
                .padding(.top, 8)
            }
        }
    }
}

/// 中英提示: the badge beside the caret after a Shift tap.
struct ModeBadgeVignette: View {
    @ObservedObject var preferences: Preferences
    var body: some View {
        let shows = preferences.showModeToast
        Vignette {
            VignetteLoop(count: 2, interval: 1.6, rest: 1) { phase in
                HStack(alignment: .center, spacing: 10) {
                    VignetteLine(committed: "王小明", marked: "")
                    KeyChip(text: CurrentKeys(preferences).modeKey ?? "Shift")
                    if shows {
                        Text(phase == 0 ? "中" : "英")
                            .font(.system(size: 15, weight: .semibold))
                            .frame(width: 30, height: 30)
                            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(.regularMaterial))
                            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(.quaternary, lineWidth: 0.5))
                            .id(phase)
                            .transition(.opacity.combined(with: .scale(scale: 0.9)))
                    } else {
                        Text("不顯示提示").font(.caption).foregroundStyle(.tertiary)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 12)
            }
        }
    }
}
