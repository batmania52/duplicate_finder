import AppKit
import Foundation

enum ReaderMode: String, CaseIterable {
    case scroll = "스크롤"
    case page = "페이지"
}

enum FitMode: String, CaseIterable {
    case width = "폭"
    case height = "높이"
    case balance = "균형"
}

enum PageLayoutMode: String, CaseIterable {
    case single = "1장"
    case double = "2장"
}

enum SourceKind {
    case image
    case folder
    case archive
}

struct ExplorerNode: Identifiable, Hashable {
    let id = UUID()
    let url: URL
    let name: String
    let isDirectory: Bool
    let isSupportedFile: Bool
}

struct ImageEntry: Identifiable, Hashable {
    let id = UUID()
    let url: URL
    let name: String
}
