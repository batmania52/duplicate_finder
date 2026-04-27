import AppKit
import SwiftUI

struct KeyboardMonitorView: NSViewRepresentable {
    let onKeyDown: (NSEvent) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onKeyDown: onKeyDown)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        context.coordinator.install()
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.uninstall()
    }

    final class Coordinator {
        let onKeyDown: (NSEvent) -> Void
        private var monitor: Any?

        init(onKeyDown: @escaping (NSEvent) -> Void) {
            self.onKeyDown = onKeyDown
        }

        func install() {
            guard monitor == nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [onKeyDown] event in
                onKeyDown(event)
                return nil
            }
        }

        func uninstall() {
            if let monitor {
                NSEvent.removeMonitor(monitor)
            }
            monitor = nil
        }
    }
}
