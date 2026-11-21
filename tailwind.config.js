/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        sans: [
          '"Segoe UI Variable Text"',
          '"Segoe UI Variable Display"',
          '"Segoe UI"',
          "-apple-system",
          "BlinkMacSystemFont",
          "system-ui",
          "sans-serif",
        ],
      },
      colors: {
        fluent: {
          bg: "#1c1c1c",
          bgLayer: "#202020",
          card: "#272727",
          cardHover: "#2e2e2e",
          cardActive: "#323232",
          border: "rgba(255, 255, 255, 0.07)",
          borderHover: "rgba(255, 255, 255, 0.12)",
          subtle: "rgba(255, 255, 255, 0.04)",
          subtleHover: "rgba(255, 255, 255, 0.07)",
          divider: "rgba(255, 255, 255, 0.06)",
          textPrimary: "#ffffff",
          textSecondary: "#cccccc",
          textTertiary: "#999999",
          textDisabled: "#666666",
          accent: "#ff7a45",
          accentHover: "#ff8c5a",
          accentBg: "rgba(255, 122, 69, 0.12)",
          accentBorder: "rgba(255, 122, 69, 0.3)",
          blue: "#60cdff",
          green: "#6ccb5f",
          yellow: "#fce100",
          red: "#ff99a4",
        },
      },
    },
  },
  plugins: [],
};

