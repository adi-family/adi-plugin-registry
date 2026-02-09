/**
 * Plugin Registry HTTP API
 *
 * Package and plugin registry with search, versioning, and artifact hosting.
 */

import "@typespec/http";

using TypeSpec.Http;

namespace PluginRegistry;

// -- Models --

model PackageEntry {
  id: string;
  name: string;
  description: string;
  pluginCount: uint32;
  pluginIds: string[];
  latestVersion: string;
  downloads: uint64;
  author: string;
  tags: string[];
}

model PluginEntry {
  id: string;
  name: string;
  description: string;
  pluginType: string;
  packageId?: string;
  latestVersion: string;
  downloads: uint64;
  author: string;
  tags: string[];
}

model PlatformBuild {
  platform: string;
  downloadUrl: string;
  sizeBytes: uint64;
  checksum: string;
  signature?: string;
}

model PackageInfo {
  id: string;
  version: string;
  platforms: PlatformBuild[];
  publishedAt: uint64;
  changelog?: string;
}

model WebUiMeta {
  entryUrl: string;
  sizeBytes: uint64;
}

model PluginInfo {
  id: string;
  version: string;
  platforms: PlatformBuild[];
  publishedAt: uint64;
  webUi?: WebUiMeta;
}

model RegistryIndex {
  version: uint32;
  updatedAt: uint64;
  packages: PackageEntry[];
  plugins: PluginEntry[];
}

model SearchResults {
  packages: PackageEntry[];
  plugins: PluginEntry[];
}

model SearchQuery {
  @query q: string;
  @query kind?: string;
}

model PublishResponse {
  status: string;
  id: string;
  version: string;
  platform: string;
}

model PublishParams {
  @query name: string;
  @query description?: string;
  @query pluginType?: string;
  @query author?: string;
}

// -- Interfaces --

interface IndexService {
  @get
  @route("/v1/index.json")
  getIndex(): {
    @statusCode statusCode: 200;
    @body body: RegistryIndex;
  };
}

interface SearchService {
  @get
  @route("/v1/search")
  search(...SearchQuery): {
    @statusCode statusCode: 200;
    @body body: SearchResults;
  };
}

@route("/v1/packages")
interface PackageService {
  @get
  @route("/{id}/latest.json")
  getLatest(@path id: string): {
    @statusCode statusCode: 200;
    @body body: PackageInfo;
  };

  @get
  @route("/{id}/{version}.json")
  getVersion(@path id: string, @path version: string): {
    @statusCode statusCode: 200;
    @body body: PackageInfo;
  };

  @get
  @route("/{id}/{version}/{platform}.tar.gz")
  download(@path id: string, @path version: string, @path platform: string): {
    @statusCode statusCode: 200;
    @body body: bytes;
  };
}

@route("/v1/publish/packages")
interface PackagePublishService {
  @post
  @route("/{id}/{version}/{platform}")
  publish(@path id: string, @path version: string, @path platform: string, ...PublishParams, @body body: bytes): {
    @statusCode statusCode: 201;
    @body body: PublishResponse;
  };
}

@route("/v1/plugins")
interface PluginService {
  @get
  @route("/{id}/latest.json")
  getLatest(@path id: string): {
    @statusCode statusCode: 200;
    @body body: PluginInfo;
  };

  @get
  @route("/{id}/{version}.json")
  getVersion(@path id: string, @path version: string): {
    @statusCode statusCode: 200;
    @body body: PluginInfo;
  };

  @get
  @route("/{id}/{version}/{platform}.tar.gz")
  download(@path id: string, @path version: string, @path platform: string): {
    @statusCode statusCode: 200;
    @body body: bytes;
  };
}

@route("/v1/publish/plugins")
interface PluginPublishService {
  @post
  @route("/{id}/{version}/{platform}")
  publish(@path id: string, @path version: string, @path platform: string, ...PublishParams, @body body: bytes): {
    @statusCode statusCode: 201;
    @body body: PublishResponse;
  };
}

@route("/v1/publish/plugins")
interface PluginWebUiPublishService {
  @post
  @route("/{id}/{version}/web")
  publish(@path id: string, @path version: string, @body body: bytes): {
    @statusCode statusCode: 201;
    @body body: PublishResponse;
  };
}

@route("/v1/plugins")
interface PluginWebUiService {
  @get
  @route("/{id}/{version}/web.js")
  download(@path id: string, @path version: string): {
    @statusCode statusCode: 200;
    @body body: bytes;
  };
}
