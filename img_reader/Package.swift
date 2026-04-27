// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "ImgReader",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "ImgReader", targets: ["ImgReaderApp"])
    ],
    targets: [
        .executableTarget(
            name: "ImgReaderApp",
            path: "Sources/ImgReaderApp"
        )
    ]
)
