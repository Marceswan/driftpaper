const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const webpack = require('webpack');
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');
const CopyPlugin = require('copy-webpack-plugin');

module.exports = (env, argv) => {
  const skipWasmPack = env['SKIP_WASM_PACK'] ?? false;
  const elmBin = env['ELM_BIN'];

  console.log(env);

  let config = {
    entry: {
      index: './src/index.js',
      landing: './src/landing.js',
    },

    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: '[name].js',
    },

    module: {
      rules: [
        {
          test: /\.elm$/,
          exclude: [/elm-stuff/, /node_modules/],
          use: {
            loader: 'elm-webpack-loader',
            options: {
              pathToElm: elmBin,
            },
          },
        },
        {
          test: /\.css$/,
          use: ['style-loader', 'css-loader'],
        },
      ],
    },

    plugins: [
      // Main app with Elm UI controls
      new HtmlWebpackPlugin({
        template: 'src/index.html',
        filename: 'app.html',
        chunks: ['index'],
      }),

      // Landing page (minimal, no Elm)
      new HtmlWebpackPlugin({
        template: 'src/landing.html',
        filename: 'index.html',
        chunks: ['landing'],
      }),

      new CopyPlugin({
        patterns: [
          { from: 'public' },
        ],
      }),
    ],

    mode: 'development',

    experiments: {
      asyncWebAssembly: true,
    },
  };

  if (!skipWasmPack) {
    // WebGL2
    config.plugins.push(
      new WasmPackPlugin({
        crateDirectory: path.resolve(__dirname, '../flux-gl/flux-wasm'),
        watchDirectories: [
          path.resolve(__dirname, '../flux-gl'),
        ],
        outDir: path.join(__dirname, 'flux-gl'),
      }),
    );

    // WebGPU
    config.plugins.push(
      new WasmPackPlugin({
        crateDirectory: path.resolve(__dirname, '../flux-wasm'),
        watchDirectories: [
          path.resolve(__dirname, '../flux'),
          path.resolve(__dirname, '../flux-wasm'),
        ],
        outDir: path.join(__dirname, 'flux'),
      }),
    );
  }

  return config;
};
