import SwiftUI

struct FileExplorerView: View {
    @EnvironmentObject private var model: ReaderModel

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 6) {
                    if let directory = model.explorerDirectory {
                        directoryHeader(directory)
                    }
                    if folders.isEmpty && files.isEmpty {
                        emptyState
                    } else {
                        if !folders.isEmpty {
                            sectionTitle("디렉토리")
                            ForEach(Array(folders.enumerated()), id: \.element.id) { index, node in
                                explorerRow(node, absoluteIndex: foldersIndices[index])
                            }
                        }
                        if !files.isEmpty {
                            sectionTitle("이미지/파일")
                            ForEach(Array(files.enumerated()), id: \.element.id) { index, node in
                                explorerRow(node, absoluteIndex: filesIndices[index])
                            }
                        }
                        if !model.recentFiles.isEmpty {
                            Divider()
                                .padding(.vertical, 8)
                            sectionTitle("최근 파일")
                            ForEach(model.recentFiles) { record in
                                recentRow(record)
                            }
                        }
                    }
                }
                .padding(12)
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var folders: [ExplorerNode] {
        model.explorerNodes.filter { $0.isDirectory }
    }

    private var files: [ExplorerNode] {
        model.explorerNodes.filter { !$0.isDirectory }
    }

    private var foldersIndices: [Int] {
        model.explorerNodes.indices.filter { model.explorerNodes[$0].isDirectory }
    }

    private var filesIndices: [Int] {
        model.explorerNodes.indices.filter { !model.explorerNodes[$0].isDirectory }
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("file explorer")
                    .font(.headline)
                Text(model.explorerDirectory?.path ?? "대상 없음")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            Spacer()
            Button("상위") { model.goParent() }
                .disabled(model.explorerDirectory == nil)
        }
        .padding(12)
    }

    private func directoryHeader(_ url: URL) -> some View {
        HStack {
            Image(systemName: "folder")
            Text(url.lastPathComponent.isEmpty ? url.path : url.lastPathComponent)
                .font(.subheadline.weight(.semibold))
            Spacer()
        }
        .padding(.bottom, 4)
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("이미지나 폴더가 없습니다.")
                .font(.subheadline.weight(.medium))
            Text("열기 버튼으로 다른 디렉토리를 선택하세요.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 18)
    }

    private func sectionTitle(_ title: String) -> some View {
        Text(title)
            .font(.caption.weight(.semibold))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 10)
            .padding(.bottom, 4)
    }

    private func explorerRow(_ node: ExplorerNode, absoluteIndex: Int) -> some View {
        Button {
            model.explorerSelectionIndex = absoluteIndex
            model.focusExplorer()
            model.activateSelectedExplorerNode()
        } label: {
            HStack(spacing: 8) {
                Image(systemName: node.isDirectory ? "folder.fill" : "photo")
                    .foregroundStyle(node.isDirectory ? .blue : .primary)
                Text(node.name)
                    .font(.callout)
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(rowBackground(selected: absoluteIndex == model.explorerSelectionIndex))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            if hovering {
                model.explorerSelectionIndex = absoluteIndex
            }
        }
    }

    private func rowBackground(selected: Bool) -> Color {
        selected ? Color.accentColor.opacity(0.16) : Color.clear
    }

    private func recentRow(_ record: RecentFileRecord) -> some View {
        Button {
            model.focusExplorer()
            model.open(url: URL(fileURLWithPath: record.path), promptForRecent: true)
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "clock")
                    .foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text(record.displayName)
                        .font(.callout)
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    Text("마지막 위치 \(record.pageIndex + 1)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color.accentColor.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .buttonStyle(.plain)
    }
}
