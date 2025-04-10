# CHANGELOG (svg2png)


<a name="v0.2.1"></a>
## [v0.2.1](https://github.com/Govcraft/svg2png/compare/v0.2.0...v0.2.1)

> 2025-04-10


<a name="v0.2.0"></a>
## [v0.2.0](https://github.com/Govcraft/svg2png/compare/v0.1.5...v0.2.0)

> 2025-04-10

### Features

* **api:** add PNG transparency endpoint using ImageMagick


<a name="v0.1.5"></a>
## [v0.1.5](https://github.com/Govcraft/svg2png/compare/v0.1.4...v0.1.5)

> 2025-04-07

### Bug Fixes

* **font:** resolve font warnings in container


<a name="v0.1.4"></a>
## [v0.1.4](https://github.com/Govcraft/svg2png/compare/v0.1.3...v0.1.4)

> 2025-04-07

### Bug Fixes

* **docker:** install fontconfig and rebuild font cache

### Code Refactoring

* **usvg:** explicitly set default font family in options


<a name="v0.1.3"></a>
## [v0.1.3](https://github.com/Govcraft/svg2png/compare/v0.1.2...v0.1.3)

> 2025-04-07

### Bug Fixes

* **docker:** install Times New Roman font and run as non-root user


<a name="v0.1.2"></a>
## [v0.1.2](https://github.com/Govcraft/svg2png/compare/v0.1.1...v0.1.2)

> 2025-04-07


<a name="v0.1.1"></a>
## v0.1.1

> 2025-04-07

### Bug Fixes

* **api:** prevent panic on empty request body
* **render:** embed correct DPI metadata in PNG output
* **render:** correct SVG rendering logic and dependencies

### Code Refactoring

* replace magic strings with constants
* **main:** replace unwraps with anyhow error handling
* **render:** use usvg::Options.dpi for scaling

### Features

* **api:** add /health endpoint
* **deploy:** add Dockerfile for static MUSL build and scratch deployment
* **render:** add DPI scaling via query parameter
* **server:** implement graceful shutdown

