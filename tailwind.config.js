/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: [
    "./index.html",
    "./src/**/*.{ts,tsx,js,jsx}",
  ],
  theme: {
    extend: {
      colors: {
        // CheersAI 品牌主色 - 统一使用浅蓝色系
        primary: {
          DEFAULT: '#3b82f6',  // blue-500
          dark: '#2563eb',     // blue-600
          light: '#60a5fa',    // blue-400
          foreground: '#ffffff',
        },
        // 功能色
        success: {
          DEFAULT: '#3b82f6',  // 统一使用蓝色
          foreground: '#ffffff',
        },
        warning: {
          DEFAULT: '#f59e0b',  // 保留黄色用于警告
          foreground: '#ffffff',
        },
        error: {
          DEFAULT: '#ef4444',  // 保留红色用于错误
          foreground: '#ffffff',
        },
        info: {
          DEFAULT: '#3b82f6',  // 统一使用蓝色
          foreground: '#ffffff',
        },
        // 中性色
        gray: {
          50: '#f9fafb',
          100: '#f3f4f6',
          200: '#e5e7eb',
          300: '#d1d5db',
          400: '#9ca3af',
          500: '#6b7280',
          600: '#4b5563',
          700: '#374151',
          800: '#1f2937',
          900: '#111827',
        },
        // 兼容 shadcn/ui
        border: '#e5e7eb',
        input: '#e5e7eb',
        ring: '#3b82f6',
        background: '#ffffff',
        foreground: '#111827',
        secondary: {
          DEFAULT: '#f3f4f6',
          foreground: '#111827',
        },
        destructive: {
          DEFAULT: '#ef4444',
          foreground: '#ffffff',
        },
        muted: {
          DEFAULT: '#f3f4f6',
          foreground: '#6b7280',
        },
        accent: {
          DEFAULT: '#f3f4f6',
          foreground: '#111827',
        },
        popover: {
          DEFAULT: '#ffffff',
          foreground: '#111827',
        },
        card: {
          DEFAULT: '#ffffff',
          foreground: '#111827',
        },
      },
      borderRadius: {
        sm: '0.125rem',   // 2px
        DEFAULT: '0.25rem',  // 4px
        md: '0.375rem',   // 6px
        lg: '0.5rem',     // 8px
        xl: '0.75rem',    // 12px
        '2xl': '1rem',    // 16px
        full: '9999px',
      },
      boxShadow: {
        sm: '0 1px 2px rgba(0, 0, 0, 0.05)',
        DEFAULT: '0 1px 3px rgba(0, 0, 0, 0.1)',
        md: '0 4px 6px rgba(0, 0, 0, 0.1)',
        lg: '0 10px 15px rgba(0, 0, 0, 0.1)',
        xl: '0 20px 25px rgba(0, 0, 0, 0.1)',
        '2xl': '0 25px 50px rgba(0, 0, 0, 0.15)',
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'Helvetica Neue', 'Arial', 'sans-serif'],
      },
      fontSize: {
        xs: ['0.625rem', { lineHeight: '1.3' }],   // 10px
        sm: ['0.75rem', { lineHeight: '1.4' }],    // 12px
        base: ['0.875rem', { lineHeight: '1.5' }], // 14px
        lg: ['1rem', { lineHeight: '1.5' }],       // 16px
        xl: ['1.125rem', { lineHeight: '1.4' }],   // 18px
        '2xl': ['1.25rem', { lineHeight: '1.3' }], // 20px
        '3xl': ['1.5rem', { lineHeight: '1.2' }],  // 24px
      },
      transitionDuration: {
        150: '150ms',
        200: '200ms',
        250: '250ms',
        300: '300ms',
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
        "pulse-dot": {
          '0%, 100%': { opacity: '0.4' },
          '50%': { opacity: '1' },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
        "pulse-dot": "pulse-dot 1.4s ease-in-out infinite",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};
