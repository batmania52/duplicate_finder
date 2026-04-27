import SwiftUI

struct ReaderDetailView: View {
    @EnvironmentObject private var model: ReaderModel

    var body: some View {
        VStack(spacing: 0) {
            readerHeader
            Divider()
            content
        }
        .background(Color(nsColor: .textBackgroundColor))
        .onTapGesture {
            model.focusViewer()
        }
    }

    private var readerHeader: some View {
        HStack(spacing: 12) {
            Text(model.statusMessage)
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
            Text(model.readerMode.rawValue)
                .font(.caption.weight(.medium))
                .foregroundStyle(.secondary)
            Text(model.pageLayout.rawValue)
                .font(.caption.weight(.medium))
                .foregroundStyle(.secondary)
            Text(model.fitMode.rawValue)
                .font(.caption.weight(.medium))
                .foregroundStyle(.secondary)
        }
        .padding(12)
    }

    @ViewBuilder
    private var content: some View {
        if model.readerEntries.isEmpty {
            emptyReader
        } else {
            GeometryReader { proxy in
                switch model.readerMode {
                case .scroll:
                    ScrollViewReader { scrollProxy in
                        ScrollView(.vertical) {
                            LazyVStack(spacing: 0) {
                                ForEach(Array(model.readerEntries.enumerated()), id: \.offset) { index, entry in
                                    scrollItem(entry: entry, index: index, containerSize: proxy.size)
                                        .id(index)
                                }
                            }
                        }
                        .onAppear {
                            scheduleScrollToCurrent(scrollProxy)
                        }
                        .onChange(of: model.currentIndex) { _, _ in
                            scheduleScrollToCurrent(scrollProxy)
                        }
                        .onChange(of: model.readerSessionID) { _, _ in
                            scheduleScrollToCurrent(scrollProxy)
                        }
                    }
                    .id(model.readerSessionID)
                case .page:
                    if model.pageLayout == .double {
                        doublePageView(containerSize: proxy.size)
                    } else if let image = model.currentImage {
                        ZStack {
                            Color.clear
                            imageView(image)
                                .frame(width: imageFrame(in: proxy.size).width, height: imageFrame(in: proxy.size).height)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .padding(24)
                    } else {
                        emptyReader
                    }
                }
            }
        }
    }

    private func scrollToCurrent(_ proxy: ScrollViewProxy) {
        guard model.readerEntries.indices.contains(model.currentIndex) else { return }
        withAnimation(.easeInOut(duration: 0.18)) {
            proxy.scrollTo(model.currentIndex, anchor: .top)
        }
    }

    private func scheduleScrollToCurrent(_ proxy: ScrollViewProxy) {
        DispatchQueue.main.async {
            scrollToCurrent(proxy)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            scrollToCurrent(proxy)
        }
    }

    private func scrollItem(entry: ImageEntry, index: Int, containerSize: CGSize) -> some View {
        return imageViewForEntry(entry, containerSize: containerSize)
            .frame(maxWidth: .infinity)
            .accessibilityLabel(Text(entry.name))
            .accessibilityValue(Text("\(index + 1)"))
            .onTapGesture {
                model.currentIndex = index
                model.focusViewer()
            }
        }

    private func imageViewForEntry(_ entry: ImageEntry, containerSize: CGSize) -> some View {
        Group {
            if let image = NSImage(contentsOf: entry.url) {
                let frame = imageFrame(in: containerSize, image: image)
                imageView(image)
                    .frame(width: frame.width, height: frame.height)
                    .frame(maxWidth: .infinity)
            } else {
                RoundedRectangle(cornerRadius: 14)
                    .fill(Color.secondary.opacity(0.12))
                    .frame(height: 220)
                    .overlay {
                        Text("이미지를 불러올 수 없습니다")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
            }
        }
    }

    private func imageView(_ image: NSImage) -> some View {
        Image(nsImage: image)
            .resizable()
            .interpolation(.high)
            .aspectRatio(contentMode: .fit)
            .shadow(radius: 12, y: 6)
    }

    @ViewBuilder
    private func doublePageView(containerSize: CGSize) -> some View {
        if model.readerEntries.isEmpty {
            emptyReader
        } else {
            let currentEntry = model.readerEntries.indices.contains(model.currentIndex) ? model.readerEntries[model.currentIndex] : nil
            let nextEntry = model.readerEntries.indices.contains(model.currentIndex + 1) ? model.readerEntries[model.currentIndex + 1] : nil
            HStack(spacing: 16) {
                if let currentEntry {
                    imageViewForEntry(currentEntry, containerSize: CGSize(width: containerSize.width / 2 - 24, height: containerSize.height))
                }
                if let nextEntry {
                    imageViewForEntry(nextEntry, containerSize: CGSize(width: containerSize.width / 2 - 24, height: containerSize.height))
                } else {
                    Spacer(minLength: 0)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(24)
        }
    }

    private var emptyReader: some View {
        VStack(spacing: 12) {
            Image(systemName: "photo.on.rectangle.angled")
                .font(.system(size: 40))
                .foregroundStyle(.secondary)
            Text("이미지를 열어주세요")
                .font(.title3.weight(.semibold))
            Text("왼쪽 탐색기에서 이미지를 고르거나 Finder에서 끌어오면 됩니다.")
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func imageFrame(in size: CGSize, image: NSImage? = nil) -> CGSize {
        let sourceImage = image ?? model.currentImage
        guard let image = sourceImage else { return .zero }
        let imageSize = image.size
        guard imageSize.width > 0, imageSize.height > 0 else { return .zero }

        switch model.fitMode {
        case .width:
            let width = max(size.width, 100)
            let height = width * imageSize.height / imageSize.width
            return CGSize(width: width * model.zoomScale, height: height * model.zoomScale)
        case .height:
            let height = max(size.height, 100)
            let width = height * imageSize.width / imageSize.height
            return CGSize(width: width * model.zoomScale, height: height * model.zoomScale)
        case .balance:
            let scale = min(size.width / imageSize.width, size.height / imageSize.height)
            let clamped = max(scale, 0.05)
            let width = imageSize.width * clamped
            let height = imageSize.height * clamped
            return CGSize(width: width * model.zoomScale, height: height * model.zoomScale)
        }
    }
}
