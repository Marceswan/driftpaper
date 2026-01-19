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

async function initFlux() {
  const canvas = document.getElementById("canvas");

  // Debug canvas dimensions
  console.log("Canvas clientWidth:", canvas.clientWidth, "clientHeight:", canvas.clientHeight);
  console.log("Window dimensions:", window.innerWidth, window.innerHeight);

  // If canvas has no CSS dimensions yet, force them
  if (canvas.clientWidth === 0 || canvas.clientHeight === 0) {
    console.log("Canvas has no dimensions, setting explicitly");
    canvas.style.width = window.innerWidth + "px";
    canvas.style.height = window.innerHeight + "px";
    // Force reflow
    canvas.offsetHeight;
    console.log("After fix - clientWidth:", canvas.clientWidth, "clientHeight:", canvas.clientHeight);
  }

  // Set canvas buffer size to match display size
  canvas.width = canvas.clientWidth * window.devicePixelRatio;
  canvas.height = canvas.clientHeight * window.devicePixelRatio;
  console.log("Canvas buffer size:", canvas.width, canvas.height);

  try {
    // Check WebGPU support
    let hasWebGPU = false;
    try {
      hasWebGPU = navigator.gpu && await navigator.gpu.requestAdapter();
    } catch (e) {
      console.log("WebGPU not available:", e);
    }

    if (hasWebGPU) {
      console.log("Backend: WebGPU");
      const wasm = await import(/* webpackIgnore: true */ "/flux/flux_wasm.js");
      await wasm.default("/flux/flux_wasm_bg.wasm");
      flux = await new wasm.Flux(settings);
    } else {
      console.log("Backend: WebGL2");
      console.log("WebGL2RenderingContext available:", typeof WebGL2RenderingContext !== "undefined");

      const wasm = await import(/* webpackIgnore: true */ "/flux-gl/flux_gl_wasm.js");
      await wasm.default("/flux-gl/flux_gl_wasm_bg.wasm");
      console.log("WASM loaded, creating Flux with settings:", JSON.stringify(settings));
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
