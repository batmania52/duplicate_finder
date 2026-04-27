import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var model: ReaderModel
    @State private var sidebarVisibility: NavigationSplitViewVisibility = .all

    var body: some View {
        NavigationSplitView(columnVisibility: $sidebarVisibility) {
            if model.isSidebarVisible {
                FileExplorerView()
                    .frame(minWidth: 280, idealWidth: 320, maxWidth: .infinity)
            } else {
                Color.clear.frame(width: 8)
            }
        } detail: {
            ReaderDetailView()
        }
        .overlay(alignment: .bottomLeading) {
            if model.isLoading {
                LoadingPill(text: model.statusMessage)
                    .padding(.leading, 16)
                    .padding(.bottom, 16)
            }
        }
        .background(
            KeyboardMonitorView { event in
                handleKey(event)
            }
            .frame(width: 0, height: 0)
        )
        .onChange(of: sidebarVisibility) { _, newValue in
            model.isSidebarVisible = newValue != .detailOnly
        }
        .toolbar {
            ToolbarItemGroup(placement: .automatic) {
                Button("열기") { model.openPickedItem() }
                Button {
                    toggleSidebar()
                } label: {
                    Label(model.isSidebarVisible ? "탐색기 닫기" : "탐색기 열기", systemImage: "sidebar.leading")
                }
                Button("이전 파일") { model.previousFile() }
                Button("다음 파일") { model.nextFile() }
                Button("축소") { model.zoomOut() }
                Button("원본") { model.resetZoom() }
                Button("확대") { model.zoomIn() }
                Picker("모드", selection: $model.readerMode) {
                    ForEach(ReaderMode.allCases, id: \.self) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 160)
                Picker("보기", selection: $model.pageLayout) {
                    ForEach(PageLayoutMode.allCases, id: \.self) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 120)
                Picker("맞춤", selection: $model.fitMode) {
                    ForEach(FitMode.allCases, id: \.self) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 180)
            }
        }
    }

    private func toggleSidebar() {
        let nextVisible = !model.isSidebarVisible
        model.isSidebarVisible = nextVisible
        sidebarVisibility = nextVisible ? .all : .detailOnly
    }

    private func handleKey(_ event: NSEvent) {
        if event.modifierFlags.contains(.option) {
            switch event.keyCode {
            case 123, 116:
                if model.focusTarget == .viewer { model.jump(by: -10) }
            case 124, 121:
                if model.focusTarget == .viewer { model.jump(by: 10) }
            default:
                break
            }
            return
        }

        switch event.keyCode {
        case 123:
            if model.focusTarget == .viewer { model.previous() }
        case 124:
            if model.focusTarget == .viewer { model.next() }
        case 125:
            if model.focusTarget == .explorer { model.moveExplorerSelection(1) }
        case 126:
            if model.focusTarget == .explorer { model.moveExplorerSelection(-1) }
        case 36, 76:
            if model.focusTarget == .explorer { model.activateSelectedExplorerNode() }
        case 51:
            if model.focusTarget == .explorer { model.goParent() }
        case 24:
            if model.focusTarget == .viewer { model.zoomIn() } // = / +
        case 27:
            if model.focusTarget == .viewer { model.zoomOut() } // -
        case 29:
            if model.focusTarget == .viewer { model.resetZoom() } // 0
        default:
            break
        }
    }
}

private struct LoadingPill: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(.ultraThinMaterial)
            .clipShape(Capsule())
            .shadow(radius: 4, y: 2)
    }
}
