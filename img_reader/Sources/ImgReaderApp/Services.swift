import AppKit
import Foundation

enum ReaderError: LocalizedError {
    case unsupportedFile(URL)
    case noImages(URL)
    case extractionFailed(String)

    var errorDescription: String? {
        switch self {
        case let .unsupportedFile(url):
            return "지원하지 않는 파일입니다: \(url.lastPathComponent)"
        case let .noImages(url):
            return "이미지를 찾을 수 없습니다: \(url.lastPathComponent)"
        case let .extractionFailed(message):
            return message
        }
    }
}

final class ImageCache: @unchecked Sendable {
    private let cache = NSCache<NSString, NSImage>()

    func image(for url: URL) -> NSImage? {
        cache.object(forKey: url.path as NSString)
    }

    func set(_ image: NSImage, for url: URL) {
        cache.setObject(image, forKey: url.path as NSString)
    }

    func clear() {
        cache.removeAllObjects()
    }
}

struct DirectoryScanner {
    func scanExplorerNodes(in directory: URL) throws -> [ExplorerNode] {
        let urls = try FileManager.default.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        )

        let nodes = urls.compactMap { url -> ExplorerNode? in
            let values = try? url.resourceValues(forKeys: [.isDirectoryKey])
            let isDirectory = values?.isDirectory ?? false
            let isSupportedFile = FileSupport.isSupportedImage(url) || FileSupport.isSupportedArchive(url)

            guard isDirectory || isSupportedFile else { return nil }

            return ExplorerNode(
                url: url,
                name: url.lastPathComponent,
                isDirectory: isDirectory,
                isSupportedFile: isSupportedFile
            )
        }

        return sortNodes(nodes)
    }

    func scanImageEntries(in directory: URL, recursive: Bool) throws -> [ImageEntry] {
        let fileManager = FileManager.default
        let urls: [URL]

        if recursive {
            guard let enumerator = fileManager.enumerator(
                at: directory,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: [.skipsHiddenFiles]
            ) else {
                return []
            }

            urls = enumerator.compactMap { $0 as? URL }
        } else {
            urls = try fileManager.contentsOfDirectory(
                at: directory,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: [.skipsHiddenFiles]
            )
        }

        let imageURLs = urls.compactMap { url -> ImageEntry? in
            let values = try? url.resourceValues(forKeys: [.isDirectoryKey])
            if values?.isDirectory == true { return nil }
            guard FileSupport.isSupportedImage(url) else { return nil }
            return ImageEntry(url: url, name: url.lastPathComponent)
        }

        return imageURLs.sorted { lhs, rhs in
            lhs.name.localizedStandardCompare(rhs.name) == .orderedAscending
        }
    }

    private func sortNodes(_ nodes: [ExplorerNode]) -> [ExplorerNode] {
        nodes.sorted { lhs, rhs in
            if lhs.isDirectory != rhs.isDirectory {
                return lhs.isDirectory && !rhs.isDirectory
            }
            return lhs.name.localizedStandardCompare(rhs.name) == .orderedAscending
        }
    }
}

struct ArchiveExtractor {
    func extract(_ archiveURL: URL) throws -> URL {
        let tempRoot = FileManager.default.temporaryDirectory
        let extractDir = tempRoot.appendingPathComponent("img_reader-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: extractDir, withIntermediateDirectories: true)

        let process = Process()
        let lowerExt = archiveURL.pathExtension.lowercased()
        switch lowerExt {
        case "zip", "cbz":
            process.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
            process.arguments = ["-oq", archiveURL.path, "-d", extractDir.path]
        case "rar", "cbr":
            process.executableURL = URL(fileURLWithPath: "/usr/bin/unrar")
            process.arguments = ["x", "-o+", archiveURL.path, extractDir.path]
        default:
            throw ReaderError.unsupportedFile(archiveURL)
        }

        let pipe = Pipe()
        process.standardError = pipe
        process.standardOutput = Pipe()

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            throw ReaderError.extractionFailed("압축 해제 도구를 실행할 수 없습니다: \(error.localizedDescription)")
        }

        if process.terminationStatus != 0 {
            let errData = pipe.fileHandleForReading.readDataToEndOfFile()
            let message = String(data: errData, encoding: .utf8) ?? "압축 해제 실패"
            throw ReaderError.extractionFailed(message)
        }

        return extractDir
    }
}

struct FilePicker {
    @MainActor
    static func openFile() -> URL? {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = true
        panel.resolvesAliases = true
        panel.title = "이미지 또는 폴더 열기"
        panel.message = "이미지 파일, 폴더, 압축 파일을 선택하세요."
        if panel.runModal() == .OK {
            return panel.url
        }
        return nil
    }
}

struct RecentFileRecord: Codable, Hashable, Identifiable {
    var id: String { path }

    let path: String
    let displayName: String
    let pageIndex: Int
    let lastOpenedAt: Date
}

final class RecentFileStore {
    private enum StorageKey {
        static let recentFilesData = "img_reader.recentFilesData"
    }

    private let maxCount = 20

    func load() -> [RecentFileRecord] {
        guard let data = UserDefaults.standard.data(forKey: StorageKey.recentFilesData),
              let records = try? JSONDecoder().decode([RecentFileRecord].self, from: data) else {
            return []
        }
        return records.sorted { $0.lastOpenedAt > $1.lastOpenedAt }
    }

    func upsert(path: String, displayName: String, pageIndex: Int) {
        var records = load()
        records.removeAll { $0.path == path }
        records.insert(
            RecentFileRecord(
                path: path,
                displayName: displayName,
                pageIndex: pageIndex,
                lastOpenedAt: Date()
            ),
            at: 0
        )
        records = Array(records.prefix(maxCount))
        save(records)
    }

    func record(for path: URL) -> RecentFileRecord? {
        load().first { $0.path == path.standardizedFileURL.path }
    }

    private func save(_ records: [RecentFileRecord]) {
        if let data = try? JSONEncoder().encode(records) {
            UserDefaults.standard.set(data, forKey: StorageKey.recentFilesData)
        }
    }
}
