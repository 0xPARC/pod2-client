import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  /* config options here */
  serverExternalPackages: [
    "@pod2/pod2-node",
    "pod2-node-darwin-arm64",
    "pod2-node.darwin-arm64.node"
  ],
  webpack: (config, { isServer }) => {
    if (isServer) {
      config.externals = config.externals || [];
      config.externals.push({
        "@pod2/pod2-node": "commonjs @pod2/pod2-node"
      });
    } else {
      config.resolve.fallback = {
        ...config.resolve.alias,
        "@pod2/pod2-node": false
      };
    }
    return config;
  }
};

export default nextConfig;
