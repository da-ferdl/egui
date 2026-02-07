# xframe

## **Attention:** This is a modified copy of *[`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe): the [`egui`](https://github.com/emilk/egui) framework*

xframe is a highly opinionated and shortened version of *[`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe)*:
- Adjusted for this fork's egui UI single thread usage optimizations, with focus on macOS desktop and iOS / Android mobile apps usage 
- No support for web and access-kit
- No glow integration, only wgpu
- No `run_native` and the simple `run_native` variants that eframe has
- Only `create_native` exposed, to create your on winit application proxy
- By supporting only wgpu, backend settings are set for all platforms in xframe, so there is no need to fiddle with matching feature selections when using xframe

The reason to create the `xframe` crate inside the egui workspace instead a standalone crate:
- Standalone crate would create the impression that the substantial software portions are made by me, even with notices like 'based on' etc.
- This crate is effectively a stripped copy of `eframe` with adjustments. So inside the egui fork repository, with the original authors noted in the `Cargo.toml` it is immediately clear on which work this is based on.
- And lastly it anyway depends on the changes made to egui itself.

Be aware that this is a version of eframe adjusted for my personal needs. I use it currently for macOS desktop and iOS / Android apps, so there could be issues on other platforms that I'm not aware of.

If it fit's also your needs, great, just use it.

**But in general I recommend using the official [`egui's`](https://github.com/emilk/egui) framework [`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe)**
