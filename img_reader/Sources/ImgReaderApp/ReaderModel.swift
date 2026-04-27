import AppKit
import Foundation

@MainActor
final class ReaderModel: ObservableObject {
    private enum StorageKey {
        static let lastExplorerDirectoryPath = "img_reader.lastExplorerDirectoryPath"
    }

    private enum ReaderSourceKind {
        case directoryImages
        case archive
    }

    @Published var isSidebarVisible = true
    @Published var explorerDirectory: URL?
    @Published var explorerNodes: [ExplorerNode] = []
    @Published var explorerSelectionIndex: Int?
    @Published var readerEntries: [ImageEntry] = []
    @Published var currentIndex: Int = 0
    @Published var currentImage: NSImage?
    @Published var currentSourceURL: URL?
    @Published var isLoading = false
    @Published var statusMessage = "이미지를 열어주세요."
    @Published var readerMode: ReaderMode = .scroll
    @Published var fitMode: FitMode = .balance
    @Published var pageLayout: PageLayoutMode = .single
    @Published var zoomScale: CGFloat = 1.0
    @Published var focusTarget: FocusTarget = .explorer
    @Published var readerSessionID = UUID()
    @Published var recentFiles: [RecentFileRecord] = []

    private let cache = ImageCache()
    private let scanner = DirectoryScanner()
    private let extractor = ArchiveExtractor()
    private let recentStore = RecentFileStore()
    private var currentReaderSourceKind: ReaderSourceKind = .directoryImages

    init() {
        recentFiles = recentStore.load()
        let directory = Self.restoreExplorerDirectory()
        loadExplorer(at: directory, openFirstImage: false)
    }

    var selectedExplorerNode: ExplorerNode? {
        guard let explorerSelectionIndex, explorerNodes.indices.contains(explorerSelectionIndex) else { return nil }
        return explorerNodes[explorerSelectionIndex]
    }

    enum FocusTarget {
        case explorer
        case viewer
    }

    func openPickedItem() {
        guard let url = FilePicker.openFile() else { return }
        open(url: url, promptForRecent: true)
    }

    func open(url: URL, promptForRecent: Bool = false) {
        Task { @MainActor in
            await openAsync(url, promptForRecent: promptForRecent)
        }
    }

    private func openAsync(_ url: URL, promptForRecent: Bool) async {
        let normalized = url.standardizedFileURL
        let recentRecord = promptForRecent ? recentStore.record(for: normalized) : nil
        var initialIndex = recentRecord?.pageIndex ?? 0
        if promptForRecent, let recentRecord {
            let shouldResume = askResume(from: recentRecord)
            if !shouldResume {
                initialIndex = 0
            }
        }
        resetReaderSession()
        if normalized.hasDirectoryPath {
            focusTarget = .explorer
            loadExplorer(at: normalized, openFirstImage: true)
            return
        }

        if FileSupport.isSupportedImage(normalized) {
            focusTarget = .viewer
            openSingleImage(normalized, initialIndex: initialIndex)
            return
        }

        if FileSupport.isSupportedArchive(normalized) {
            await openArchive(normalized, initialIndex: initialIndex)
            return
        }

        statusMessage = ReaderError.unsupportedFile(normalized).localizedDescription
    }

    func navigateToDirectory(_ url: URL) {
        focusTarget = .explorer
        loadExplorer(at: url.standardizedFileURL, openFirstImage: false)
    }

    func activateSelectedExplorerNode() {
        guard let node = selectedExplorerNode else { return }
        focusTarget = .explorer
        if node.isDirectory {
            loadExplorer(at: node.url, openFirstImage: false)
        } else if FileSupport.isSupportedImage(node.url) {
            open(url: node.url, promptForRecent: true)
        } else if FileSupport.isSupportedArchive(node.url) {
            open(url: node.url, promptForRecent: true)
        }
    }

    func moveExplorerSelection(_ delta: Int) {
        guard !explorerNodes.isEmpty else { return }
        let nextIndex: Int
        if let explorerSelectionIndex {
            nextIndex = max(0, min(explorerSelectionIndex + delta, explorerNodes.count - 1))
        } else {
            nextIndex = delta > 0 ? 0 : explorerNodes.count - 1
        }
        explorerSelectionIndex = nextIndex
        focusTarget = .explorer
    }

    func goParent() {
        guard let directory = explorerDirectory else { return }
        let parent = directory.deletingLastPathComponent()
        guard parent != directory else { return }
        loadExplorer(at: parent, openFirstImage: false)
        focusTarget = .explorer
    }

    func previousFile() {
        moveFile(by: -1)
    }

    func nextFile() {
        moveFile(by: 1)
    }

    func next() {
        moveImage(by: 1)
    }

    func previous() {
        moveImage(by: -1)
    }

    func jump(by offset: Int) {
        moveImage(by: offset)
    }

    func zoomIn() {
        zoomScale = min(zoomScale + 0.1, 4.0)
    }

    func zoomOut() {
        zoomScale = max(zoomScale - 0.1, 0.2)
    }

    func resetZoom() {
        zoomScale = 1.0
    }

    func toggleSidebar() {
        isSidebarVisible.toggle()
    }

    func focusViewer() {
        focusTarget = .viewer
    }

    func focusExplorer() {
        focusTarget = .explorer
    }

    private func loadExplorer(at directory: URL, openFirstImage: Bool) {
        do {
            let nodes = try scanner.scanExplorerNodes(in: directory)
            explorerDirectory = directory
            explorerNodes = nodes
            explorerSelectionIndex = nodes.isEmpty ? nil : 0
            statusMessage = directory.path
            Self.saveExplorerDirectory(directory)

            if openFirstImage {
                let entries = try scanner.scanImageEntries(in: directory, recursive: false)
                if !entries.isEmpty {
                    setEntries(entries, initialIndex: 0, sourceKind: .directoryImages, navigationURL: entries[0].url)
                    return
                }
            }

            readerEntries = []
            currentIndex = 0
            currentImage = nil
            currentSourceURL = nil
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    private func openSingleImage(_ imageURL: URL, initialIndex: Int = 0) {
        let parent = imageURL.deletingLastPathComponent()
        do {
            explorerDirectory = parent
            explorerNodes = try scanner.scanExplorerNodes(in: parent)
            explorerSelectionIndex = explorerNodes.firstIndex(where: { $0.url == imageURL })
            let entries = try scanner.scanImageEntries(in: parent, recursive: false)
            guard !entries.isEmpty else {
                throw ReaderError.noImages(parent)
            }

            let current = entries.firstIndex(where: { $0.url == imageURL }) ?? initialIndex
            setEntries(entries, initialIndex: current, sourceKind: .directoryImages, navigationURL: imageURL)
            focusTarget = .viewer
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    private func openArchive(_ archiveURL: URL, initialIndex: Int = 0) async {
        isLoading = true
        statusMessage = "압축을 읽는 중..."
        do {
            let extractedDirectory = try extractor.extract(archiveURL)
            let entries = try scanner.scanImageEntries(in: extractedDirectory, recursive: true)
            guard !entries.isEmpty else {
                throw ReaderError.noImages(archiveURL)
            }
            explorerDirectory = archiveURL.deletingLastPathComponent()
            explorerNodes = try scanner.scanExplorerNodes(in: archiveURL.deletingLastPathComponent())
            explorerSelectionIndex = explorerNodes.firstIndex(where: { $0.url == archiveURL })
            setEntries(entries, initialIndex: initialIndex, sourceKind: .archive, navigationURL: archiveURL)
            focusTarget = .viewer
        } catch {
            statusMessage = error.localizedDescription
        }
        isLoading = false
    }

    private func setEntries(_ entries: [ImageEntry], initialIndex: Int, sourceKind: ReaderSourceKind, navigationURL: URL) {
        readerEntries = entries
        currentIndex = max(0, min(initialIndex, max(entries.count - 1, 0)))
        currentReaderSourceKind = sourceKind
        currentSourceURL = navigationURL
        currentImage = nil
        statusMessage = entries.isEmpty ? "이미지가 없습니다." : "\(currentIndex + 1) / \(entries.count)"
        loadCurrentImage()
        prefetchAroundCurrent()
        saveRecentEntry()
    }

    private func loadCurrentImage() {
        guard readerEntries.indices.contains(currentIndex) else {
            currentImage = nil
            return
        }
        let entry = readerEntries[currentIndex]
        if let cached = cache.image(for: entry.url) {
            currentImage = cached
            return
        }

        isLoading = true
        let url = entry.url
        DispatchQueue.global(qos: .userInitiated).async { [cache] in
            let image = NSImage(contentsOf: url)
            if let image {
                cache.set(image, for: url)
            }
            DispatchQueue.main.async {
                self.currentImage = image
                self.isLoading = false
            }
        }
    }

    private func prefetchAroundCurrent() {
        guard !readerEntries.isEmpty else { return }
        let indexes = [currentIndex - 2, currentIndex - 1, currentIndex + 1, currentIndex + 2]
        for index in indexes where readerEntries.indices.contains(index) {
            let url = readerEntries[index].url
            guard cache.image(for: url) == nil else { continue }
            DispatchQueue.global(qos: .utility).async { [cache] in
                if let image = NSImage(contentsOf: url) {
                    cache.set(image, for: url)
                }
            }
        }
    }

    private func moveImage(by offset: Int) {
        guard !readerEntries.isEmpty else { return }
        let nextIndex = max(0, min(currentIndex + offset, readerEntries.count - 1))
        guard nextIndex != currentIndex else { return }
        currentIndex = nextIndex
        if currentReaderSourceKind == .directoryImages {
            currentSourceURL = readerEntries[currentIndex].url
        }
        focusTarget = .viewer
        statusMessage = "\(currentIndex + 1) / \(readerEntries.count)"
        loadCurrentImage()
        prefetchAroundCurrent()
        saveRecentEntry()
    }

    private func moveFile(by offset: Int) {
        guard explorerDirectory != nil, let currentSourceURL else { return }
        let supportedFiles = explorerNodes
            .filter { $0.isSupportedFile && !$0.isDirectory }
            .map(\.url)

        guard !supportedFiles.isEmpty else { return }

        let normalizedCurrent = currentSourceURL.standardizedFileURL
        guard let currentIndex = supportedFiles.firstIndex(where: { $0.standardizedFileURL == normalizedCurrent }) else {
            return
        }

        let nextIndex = max(0, min(currentIndex + offset, supportedFiles.count - 1))
        guard nextIndex != currentIndex else { return }

        focusTarget = .explorer
        open(url: supportedFiles[nextIndex])
    }

    private func resetReaderSession() {
        readerSessionID = UUID()
        currentIndex = 0
        currentImage = nil
        currentSourceURL = nil
        readerEntries = []
        statusMessage = "이미지를 읽는 중..."
        isLoading = false
        focusTarget = .explorer
        currentReaderSourceKind = .directoryImages
    }

    private func saveRecentEntry() {
        guard let currentSourceURL else { return }
        recentStore.upsert(
            path: currentSourceURL.standardizedFileURL.path,
            displayName: FileSupport.displayName(for: currentSourceURL),
            pageIndex: currentIndex
        )
        recentFiles = recentStore.load()
    }

    private static func restoreExplorerDirectory() -> URL {
        let defaults = UserDefaults.standard
        if let path = defaults.string(forKey: StorageKey.lastExplorerDirectoryPath) {
            let url = URL(fileURLWithPath: path)
            if FileManager.default.fileExists(atPath: url.path, isDirectory: nil) {
                return url
            }
        }
        return FileManager.default.homeDirectoryForCurrentUser
    }

    private static func saveExplorerDirectory(_ directory: URL) {
        UserDefaults.standard.set(directory.standardizedFileURL.path, forKey: StorageKey.lastExplorerDirectoryPath)
    }

    private func askResume(from record: RecentFileRecord) -> Bool {
        let alert = NSAlert()
        alert.messageText = record.displayName
        alert.informativeText = "최근 읽던 위치부터 열까요?"
        alert.addButton(withTitle: "최근 위치부터")
        alert.addButton(withTitle: "처음부터")
        return alert.runModal() == .alertFirstButtonReturn
    }
}
