require 'fileutils'
require 'json'
require 'open3'
require 'time'

abort 'Usage: package.rb TAG OWNER/REPOSITORY DLL OUTPUT_DIRECTORY' unless ARGV.length == 4
tag, repository, dll, output_directory = ARGV
abort "Invalid repository: #{repository}" unless repository.match?(%r{\A[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\z})

metadata, status = Open3.capture2('cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1')
abort 'cargo metadata failed' unless status.success?
package = JSON.parse(metadata).fetch('packages').find { |entry| entry.fetch('name') == 'reaper_marker_as_key' }
version = package.fetch('version')
unless version.match?(/\A\d+\.\d+\.\d+\z/) && tag == "v#{version}"
  abort "Tag must match the stable Cargo.toml version: v#{version} (got #{tag})"
end
abort "DLL not found: #{dll}" unless File.file?(dll)

timestamp, status = Open3.capture2('git', 'show', '-s', '--format=%cI', 'HEAD')
abort 'git show failed' unless status.success?
timestamp = Time.iso8601(timestamp.strip).utc.iso8601
download = "https://github.com/#{repository}/releases/download/#{tag}"

# shortcut: publish only the current version; retain history if rollback via ReaPack is needed.
index = <<~XML
  <?xml version="1.0" encoding="utf-8"?>
  <index version="1" name="reaper_marker_as_key.dll">
    <category name="Extensions">
      <reapack name="reaper_marker_as_key.ext" type="extension" desc="Set project key signatures from markers">
        <metadata>
          <link rel="website">https://github.com/#{repository}</link>
        </metadata>
        <version name="#{version}" author="sevenc-nanashi" time="#{timestamp}">
          <source platform="win64" file="reaper_marker_as_key.dll">#{download}/reaper_marker_as_key.dll</source>
        </version>
      </reapack>
    </category>
  </index>
XML

FileUtils.mkdir_p(output_directory)
FileUtils.cp(dll, File.join(output_directory, 'reaper_marker_as_key.dll'))
File.write(File.join(output_directory, 'index.xml'), index, encoding: 'UTF-8')
