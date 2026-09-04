// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "OpenEditorMedia",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "OpenEditorMedia", type: .static, targets: ["OpenEditorMedia"]),
    ],
    targets: [
        .target(
            name: "OpenEditorMedia",
            publicHeadersPath: "include",
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("CoreMedia"),
            ]
        ),
        .testTarget(name: "OpenEditorMediaTests", dependencies: ["OpenEditorMedia"]),
    ]
)
