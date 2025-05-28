import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
    title: "SP1 Contract Call Book",
    tagline: "Powered by Succinct SP1",
    favicon: "img/favicon.ico",

    // Set the production url of your site here
    url: "https://succinctlabs.github.io/",
    baseUrl: "/sp1-contract-call",

    // GitHub pages deployment config.
    // If you aren't using GitHub pages, you don't need these.
    organizationName: "succinctlabs", // Usually your GitHub org/user name.
    projectName: "sp1-contract-call", // Usually your repo name.

    onBrokenLinks: "warn",
    onBrokenMarkdownLinks: "warn",

    // Even if you don't use internationalization, you can use this field to set
    // useful metadata like html lang. For example, if your site is Chinese, you
    // may want to replace "en" with "zh-Hans".
    i18n: {
        defaultLocale: "en",
        locales: ["en"],
    },

    presets: [
        [
            "classic",
            {
                docs: {
                    routeBasePath: "/",
                    sidebarPath: "./sidebars.ts",
                },
                blog: false,
                theme: {},
            } satisfies Preset.Options,
        ],
    ],

    themeConfig: {
        // Replace with your project's social card
        image: "img/docusaurus-social-card.jpg",
        navbar: {
            title: "SP1 Contract Call",
            items: [
                {
                    type: 'dropdown',
                    label: 'API Docs',
                    position: "left",
                    items: [
                        {
                            label: 'sp1-cc-client-executor',
                            href: 'https://succinctlabs.github.io/sp1-contract-call/api/sp1_cc_client_executor',
                        },
                        {
                            label: 'sp1-cc-host-executor',
                            href: 'https://succinctlabs.github.io/sp1-contract-call/api/sp1_cc_host_executor',
                        },
                    ],
                },
                {
                    href: "https://github.com/succinctlabs/sp1-contract-call",
                    label: "GitHub",
                    position: "right",
                },
            ],
        },
        prism: {
            theme: prismThemes.github,
            darkTheme: prismThemes.dracula,
        },
    } satisfies Preset.ThemeConfig,
};

export default config;
