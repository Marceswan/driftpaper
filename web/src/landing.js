// Landing page - minimal WASM initialization without Elm UI
import "./landing.css";

let flux;

// Hardcoded settings for the landing page background
const settings = {
  mode: "Normal",
  seed: null,
  fluidSize: 128,
  fluidFrameRate: 60.0,
  fluidTimestep: 1.0 / 60.0,
  viscosity: 5.0,
  velocityDissipation: 0.0,
  pressureMode: { ClearWith: 0.0 },
  diffusionIterations: 3,
  pressureIterations: 19,
  colorMode: { Preset: "Original" },
  lineLength: 450.0,
  lineWidth: 9.0,
  lineBeginOffset: 0.4,
  lineVariance: 0.55,
  gridSpacing: 15,
  viewScale: 1.6,
  noiseMultiplier: 0.45,
  noiseChannels: [
    { scale: 2.8, multiplier: 1.0, offsetIncrement: 0.001 },
    { scale: 15.0, multiplier: 0.7, offsetIncrement: 0.006 },
    { scale: 30.0, multiplier: 0.5, offsetIncrement: 0.012 },
  ],
};

// Replace canvas element (needed after failed WebGPU attempt taints the canvas)
function replaceCanvas() {
  const oldCanvas = document.getElementById("canvas");
  const newCanvas = document.createElement("canvas");
  newCanvas.id = "canvas";
  oldCanvas.parentNode.replaceChild(newCanvas, oldCanvas);
  return newCanvas;
}

async function initFlux() {
  let canvas = document.getElementById("canvas");
  console.log("Canvas dimensions:", canvas.clientWidth, "x", canvas.clientHeight);

  // Test what contexts we can create
  const testCtx2 = canvas.getContext("webgl2");
  console.log("WebGL2 test:", testCtx2 ? "SUCCESS" : "FAILED");

  if (!testCtx2) {
    // Try WebGL1
    const testCtx1 = canvas.getContext("webgl");
    console.log("WebGL1 test:", testCtx1 ? "SUCCESS" : "FAILED");

    // Try 2D canvas
    canvas = replaceCanvas();
    const test2d = document.getElementById("canvas").getContext("2d");
    console.log("Canvas 2D test:", test2d ? "SUCCESS" : "FAILED");

    console.error("WebGL2 is not available in your browser. Please enable hardware acceleration in Chrome settings (chrome://settings/system) or check chrome://gpu for WebGL status.");
    document.body.classList.remove("loading");
    document.body.classList.add("animation-failed");
    return; // Exit early - can't render without WebGL2
  }

  console.log("WebGL2 Renderer:", testCtx2.getParameter(testCtx2.RENDERER));
  console.log("WebGL2 Vendor:", testCtx2.getParameter(testCtx2.VENDOR));

  try {
    // Check WebGPU support - but actually try to get a device, not just adapter
    let hasWebGPU = false;
    try {
      if (navigator.gpu) {
        const adapter = await navigator.gpu.requestAdapter();
        if (adapter) {
          const device = await adapter.requestDevice();
          if (device) {
            hasWebGPU = true;
            device.destroy(); // Clean up test device
          }
        }
      }
    } catch (e) {
      console.log("WebGPU not available:", e);
    }

    if (hasWebGPU) {
      console.log("Backend: WebGPU");
      // Need fresh canvas since we tested WebGL2 above
      canvas = replaceCanvas();
      const wasm = await import(/* webpackIgnore: true */ "/flux/flux_wasm.js");
      await wasm.default("/flux/flux_wasm_bg.wasm");
      try {
        flux = await new wasm.Flux(settings);
      } catch (webgpuError) {
        console.log("WebGPU Flux creation failed, falling back to WebGL2:", webgpuError);
        canvas = replaceCanvas();
        hasWebGPU = false;
      }
    }

    if (!hasWebGPU && !flux) {
      console.log("Backend: WebGL2");
      // Need fresh canvas if we did any testing above
      canvas = replaceCanvas();

      const wasm = await import(/* webpackIgnore: true */ "/flux-gl/flux_gl_wasm.js");
      await wasm.default("/flux-gl/flux_gl_wasm_bg.wasm");
      console.log("WASM loaded, creating Flux...");
      flux = new wasm.Flux(settings);
      console.log("Flux created successfully");
    }

    // Animation loop
    function animate(timestamp) {
      flux.animate(timestamp);
      window.requestAnimationFrame(animate);
    }

    // Handle canvas resize (WASM handles the actual canvas buffer sizing)
    const resizeObserver = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      if (width > 0 && height > 0) {
        flux.resize(width, height);
      }
    });
    resizeObserver.observe(canvas);

    // Start animation
    window.requestAnimationFrame(animate);

    // Mark as ready
    document.body.classList.remove("loading");
    document.body.classList.add("animation-ready");
    console.log("Drift animation initialized successfully");
  } catch (error) {
    console.error("Failed to initialize Drift animation:", error);
    // Use animated gradient fallback
    document.body.classList.remove("loading");
    document.body.classList.add("animation-failed");
  }
}

// Initialize after layout is ready
function startInit() {
  // Wait for next frame to ensure CSS is applied and canvas has dimensions
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      initFlux();
    });
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", startInit);
} else {
  startInit();
}
