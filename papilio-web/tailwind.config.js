/** @type {import('tailwindcss').Config} */
module.exports = {
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {
      colors: {
        papilio: {
          bg: "#05070A",
          surface: "#0F172A",
          accent: "#8B5CF6", // 紫罗兰
          cyan: "#06B6D4",   // 极光青
          muted: "#94A3B8",
        }
      },
      backdropBlur: {
        xs: '2px',
      }
    },
  },
  plugins: [],
}
