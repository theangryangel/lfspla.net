module.exports = {
  // Resolve locally for npm and through NODE_PATH in the isolated prek hook.
  plugins: [require.resolve("prettier-plugin-svelte")],
  overrides: [
    { files: "*.svelte", options: { useTabs: true, singleQuote: true } },
  ],
};
