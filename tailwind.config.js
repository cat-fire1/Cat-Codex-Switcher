/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        cat: {
          accent: "#ff7a45",
          accentHover: "#f86a32",
        },
        surface: {
          base: "#0d0f14",
          card: "#151821",
          cardHover: "#1b202c",
          border: "#252b3d",
          borderHover: "#39435f",
          subtle: "#1b1f2b",
        },
      },
    },
  },
  plugins: [],
};
