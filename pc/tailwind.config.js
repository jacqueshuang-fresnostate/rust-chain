/** @type {import('tailwindcss').Config} */
import animate from "tailwindcss-animate"

export default {
  darkMode: ["class"],
  content: [
    './pages/**/*.{ts,tsx,vue}',
    './components/**/*.{ts,tsx,vue}',
    './app/**/*.{ts,tsx,vue}',
    './src/**/*.{ts,tsx,vue}',
  ],
  prefix: "",
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        // Cyberpunk / Financial Colors
        'neon-blue': '#00f3ff',
        'neon-pink': '#ff00ff',
        'neon-green': '#00ff9f',
        'neon-yellow': '#fcee0a',
        'cyber-black': '#050505',
        'cyber-gray': '#121212',

        // Semantic Financial
        'up': '#00ff9f', // Neon Green for up
        'down': '#ff0055', // Neon Red/Pink for down
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: 0 },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: 0 },
        },
        "neon-pulse": {
          "0%, 100%": { boxShadow: "0 0 5px #00f3ff, 0 0 10px #00f3ff" },
          "50%": { boxShadow: "0 0 20px #00f3ff, 0 0 30px #00f3ff" },
        }
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
        "neon-pulse": "neon-pulse 2s infinite",
      },
      boxShadow: {
        'neon': '0 0 5px theme("colors.neon-blue"), 0 0 20px theme("colors.neon-blue")',
        'neon-pink': '0 0 5px theme("colors.neon-pink"), 0 0 20px theme("colors.neon-pink")',
        'neon-green': '0 0 5px theme("colors.neon-green"), 0 0 20px theme("colors.neon-green")',
      }
    },
  },
  plugins: [animate],
}
