// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "tauri-plugin-thumbnail",
    platforms: [
        .iOS(.v13)
    ],
    products: [
        .library(
            name: "tauri-plugin-thumbnail",
            type: .static,
            targets: ["tauri-plugin-thumbnail"])
    ],
    dependencies: [
        .package(url: "https://github.com/tauri-apps/tauri-plugin-ios", branch: "v2")
    ],
    targets: [
        .target(
            name: "tauri-plugin-thumbnail",
            dependencies: [
                .product(name: "Tauri", package: "tauri-plugin-ios")
            ],
            path: "Sources")
    ]
)
