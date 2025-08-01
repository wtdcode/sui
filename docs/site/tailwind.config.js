// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

const defaultTheme = require("tailwindcss/defaultTheme");

module.exports = {
  corePlugins: {
    preflight: false, // disable Tailwind's reset
  },
  content: ["./src/**/*.{js,jsx,ts,tsx}", "./docs/**/*.mdx"], // my markdown stuff is in ../docs, not /src
  darkMode: ["class", '[data-theme="dark"]'], // hooks into docusaurus' dark mode settings
  theme: {
    extend: {
      fontFamily: {
        sans: ["Inter", ...defaultTheme.fontFamily.sans],
        twkeverett: ["Twkeverett"],
      },
      colors: {
        "sui-black": "var(--sui-black)",
        "sui-blue-primary": "rgb(var(--sui-blue-primary)/<alpha-value>)",
        "sui-blue": "var(--sui-blue)",
        "sui-blue-bright": "rgb(var(--sui-blue-bright)/<alpha-value>)",
        "sui-blue-light": "rgb(var(--sui-blue-light)/<alpha-value>)",
        "sui-blue-lighter": "var(--sui-blue-lighter)",
        "sui-blue-dark": "rgb(var(--sui-blue-dark)/<alpha-value>)",
        "sui-blue-darker": "var(--sui-blue-darker)",
        "sui-hero": "var(--sui-hero)",
        "sui-hero-dark": "var(--sui-hero-dark)",
        "sui-steel": "var(--sui-steel)",
        "sui-steel-dark": "var(--sui-steel-dark)",
        "sui-steel-darker": "var(--sui-steel-darker)",
        "sui-header-nav": "var(--sui-header-nav)",
        "sui-success": "var(--sui-success)",
        "sui-success-dark": "var(--sui-success-dark)",
        "sui-success-light": "var(--sui-success-light)",
        "sui-issue": "var(--sui-issue)",
        "sui-issue-dark": "var(--sui-issue-dark)",
        "sui-issue-light": "var(--sui-issue-light)",
        "sui-warning": "var(--sui-warning)",
        "sui-warning-dark": "var(--sui-warning-dark)",
        "sui-warning-light": "var(--sui-warning-light)",
        "sui-code": "var(--sui-code)",
        "sui-gray-3s": "rgb(var(--sui-gray-3s)/<alpha-value>)",
        "sui-gray-5s": "rgb(var(--sui-gray-5s)/<alpha-value>)",
        "sui-gray": {
          35: "rgb(var(--sui-gray-35)/<alpha-value>)",
          40: "rgb(var(--sui-gray-40)/<alpha-value>)",
          45: "rgb(var(--sui-gray-45)/<alpha-value>)",
          50: "var(--sui-gray-50)",
          55: "rgb(var(--sui-gray-55)/<alpha-value>)",
          60: "var(--sui-gray-60)",
          65: "var(--sui-gray-65)",
          70: "var(--sui-gray-70)",
          75: "var(--sui-gray-75)",
          80: "var(--sui-gray-80)",
          85: "var(--sui-gray-85)",
          90: "var(--sui-gray-90)",
          95: "var(--sui-gray-95)",
          100: "var(--sui-gray-100)",
        },
        "sui-grey": {
          35: "rgb(var(--sui-gray-35)/<alpha-value>)",
          40: "rgb(var(--sui-gray-40)/<alpha-value>)",
          45: "rgb(var(--sui-gray-45)/<alpha-value>)",
          50: "var(--sui-gray-50)",
          55: "rgb(var(--sui-gray-55)/<alpha-value>)",
          60: "var(--sui-gray-60)",
          65: "var(--sui-gray-65)",
          70: "var(--sui-gray-70)",
          75: "var(--sui-gray-75)",
          80: "var(--sui-gray-80)",
          85: "var(--sui-gray-85)",
          90: "var(--sui-gray-90)",
          95: "var(--sui-gray-95)",
          100: "var(--sui-gray-100)",
        },
        "sui-disabled": "rgb(var(--sui-disabled)/<alpha-value>)",
        "sui-link-color-dark": "var(--sui-link-color-dark)",
        "sui-link-color-light": "var(--sui-link-color-light)",
        "sui-ghost-white": "var(--sui-ghost-white)",
        "sui-ghost-dark": "var(--sui-ghost-dark)",
        "ifm-background-color-dark": "var(--ifm-background-color-dark)",
        "sui-white": "rgb(var(--sui-white)/<alpha-value>)",
        "sui-card-dark": "rgb(var(--sui-card-dark)/<alpha-value>)",
        "sui-card-darker": "rgb(var(--sui-card-darker)/<alpha-value>)",
      },
      borderRadius: {
        sui: "40px",
      },
      boxShadow: {
        sui: "0px 0px 4px rgba(0, 0, 0, 0.02)",
        "sui-button": "0px 1px 2px rgba(16, 24, 40, 0.05)",
        "sui-notification": "0px 0px 20px rgba(29, 55, 87, 0.11)",
      },
      gradientColorStopPositions: {
        36: "36%",
      },
    },
  },
  plugins: [],
};
