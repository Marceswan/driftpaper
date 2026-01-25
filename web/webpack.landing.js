// Webpack config for landing page only (for Vercel deployment)
// Uses pre-built WASM, no Elm required
const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const CopyPlugin = require('copy-webpack-plugin');

module.exports = {
  entry: './src/landing.js',

  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: 'landing.js',
    clean: true,
  },

  module: {
    rules: [
      {
        test: /\.css$/,
        use: ['style-loader', 'css-loader'],
      },
    ],
  },

  plugins: [
    new HtmlWebpackPlugin({
      template: 'src/landing.html',
      filename: 'index.html',
    }),

    new CopyPlugin({
      patterns: [
        { from: 'public' },
        // Copy pre-built WASM files
        { from: 'flux/flux_wasm.js', to: 'flux/' },
        { from: 'flux/flux_wasm_bg.wasm', to: 'flux/' },
        { from: 'flux-gl/flux_gl_wasm.js', to: 'flux-gl/' },
        { from: 'flux-gl/flux_gl_wasm_bg.wasm', to: 'flux-gl/' },
      ],
    }),
  ],

  mode: 'production',

  // Suppress asset size warnings for WASM files
  performance: {
    hints: false,
  },

  devServer: {
    client: {
      overlay: {
        warnings: false,
        errors: true,
      },
    },
  },

  experiments: {
    asyncWebAssembly: true,
  },

  resolve: {
    extensions: ['.js', '.wasm'],
  },
};
