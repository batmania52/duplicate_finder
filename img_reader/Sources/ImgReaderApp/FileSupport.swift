import Foundation

enum FileSupport {
    static let imageExtensions: Set<String> = [
        "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "heic", "avif"
    ]

    static let archiveExtensions: Set<String> = [
        "zip", "cbz", "rar", "cbr"
    ]

    static func isSupportedImage(_ url: URL) -> Bool {
        imageExtensions.contains(url.pathExtension.lowercased())
    }

    static func isSupportedArchive(_ url: URL) -> Bool {
        archiveExtensions.contains(url.pathExtension.lowercased())
    }

    static func displayName(for url: URL) -> String {
        if url.hasDirectoryPath {
            return url.lastPathComponent.isEmpty ? url.path : url.lastPathComponent
        }
        return url.deletingPathExtension().lastPathComponent
    }
}
