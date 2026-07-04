// swift-tools-version:6.0
import PackageDescription

let package = Package(
    name: "AxolotlDB",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .library(
            name: "AxolotlDB",
            targets: ["GraphDatabase"]),
    ],
    targets: [
        .target(
            name: "GraphDatabase",
            dependencies: []),
        .testTarget(
            name: "GraphDatabaseTests",
            dependencies: ["GraphDatabase"]),
        .executableTarget(
            name: "Examples",
            dependencies: ["GraphDatabase"]),
    ]
)
