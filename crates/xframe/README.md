# xframe

## **Attention:** This is a modified copy of *[`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe): the [`egui`](https://github.com/emilk/egui) framework*

xframe is a highly opinionated and shortened version of *[`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe)*:
- Adjusted for this fork's egui UI single thread usage optimizations, with focus on macOS desktop and iOS / Android mobile apps usage 
- No support for web and access-kit
- No glow integration, only wgpu
- By supporting only wgpu, backend settings are set for all platforms in xframe, so there is no need to fiddle with matching feature selections when using xframe

Be aware that this is a version of eframe adjusted for my personal needs. I use it currently for macOS desktop and iOS / Android apps, so there could be issues on other platforms that I'm not aware of.

If it fit's also your needs, great, just use it.

**But in general I recommend using the official [`egui's`](https://github.com/emilk/egui) framework [`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe)**
