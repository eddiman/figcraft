import XCTest
import Vision

final class IPadSmokeTests: XCTestCase {
    private var app: XCUIApplication!
    private var originalOrientation = UIDeviceOrientation.unknown

    override func setUpWithError() throws {
        continueAfterFailure = false
        let identifier = try XCTUnwrap(ProcessInfo.processInfo.environment["FIGCRAFT_TEST_APP_ID"])
        app = XCUIApplication(bundleIdentifier: identifier)
        originalOrientation = XCUIDevice.shared.orientation
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launchArguments = ["--smoke"]
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
    }

    override func tearDown() {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "Final simulator screen"
        attachment.lifetime = .keepAlways
        add(attachment)
        app?.terminate()
        if originalOrientation != .unknown {
            XCUIDevice.shared.orientation = originalOrientation
        }
    }

    // The editor is rendered by egui; screen recognition avoids requiring an
    // iPad accessibility bridge merely to automate this visual smoke test.
    private func text(_ expected: String, minimumX: CGFloat = 0, timeout: TimeInterval = 15) throws -> CGRect {
        let deadline = Date().addingTimeInterval(timeout)
        var observed: [String] = []
        repeat {
            let screenshot = XCUIScreen.main.screenshot().image
            let image = try XCTUnwrap(screenshot.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.recognitionLanguages = ["en-US"]
            request.usesLanguageCorrection = false
            request.regionOfInterest = CGRect(x: minimumX, y: 0, width: 1 - minimumX, height: 1)
            // CoreSimulator can return portrait pixel storage for a landscape
            // screenshot. XCTest coordinates use the displayed orientation.
            let orientation: CGImagePropertyOrientation = image.height > image.width ? .left : .up
            try VNImageRequestHandler(cgImage: image, orientation: orientation).perform([request])
            observed = request.results?.compactMap { $0.topCandidates(1).first?.string } ?? []
            if let match = request.results?.compactMap({ $0.topCandidates(1).first }).first(where: {
                $0.string.range(of: expected, options: .regularExpression) != nil
            }), let range = match.string.range(of: expected, options: .regularExpression) {
                let box = try XCTUnwrap(match.boundingBox(for: range)).boundingBox
                return CGRect(
                    x: (minimumX + box.minX * (1 - minimumX)) * app.frame.width + app.frame.minX,
                    y: (1 - box.maxY) * app.frame.height + app.frame.minY,
                    width: box.width * (1 - minimumX) * app.frame.width,
                    height: box.height * app.frame.height
                )
            }
            Thread.sleep(forTimeInterval: 0.3)
        } while Date() < deadline
        XCTFail("Rendered text not found: \(expected); observed: \(observed)")
        throw NSError(domain: "IPadSmokeTests", code: 1)
    }

    private func tap(_ rect: CGRect, offsetX: CGFloat = 0) {
        app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(
            dx: rect.midX + offsetX - app.frame.minX,
            dy: rect.midY - app.frame.minY
        )).tap()
    }

    func test_ipad_editor() throws {
        _ = try text("startup smoke")
        _ = try text("\\b[0O] characters\\b")
        tap(try text("Fixture image"))
        tap(try text("Field:"), offsetX: 60)
        app.typeText("abc")
        _ = try text("3 characters")
        _ = try text("abc")
        app.typeText(String(repeating: "defghijk", count: 8))
        _ = try text("67 characters")
        tap(try text("Set fixture X = 10"))
        tap(try text("Fixture image"))
        _ = try text("\\b10\\b", minimumX: 0.65)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "Typed field and edited fixture"
        attachment.lifetime = .keepAlways
        add(attachment)
        // Allow the existing periodic recovery worker to publish before a cold
        // restart. The post-restart inspector assertion, not this wait, is proof.
        Thread.sleep(forTimeInterval: 6)
        app.terminate()
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        _ = try text("startup smoke")
        _ = try text("\\b[0O] characters\\b")
        tap(try text("Fixture image"))
        _ = try text("\\b10\\b", minimumX: 0.65)
    }

    func test_ipad_forced_failure() throws {
        _ = try text("startup smoke")
        XCTFail("Deliberate T23 assertion failure")
    }
}
