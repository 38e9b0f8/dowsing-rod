import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

def config = new File(basedir, 'dowsing-rod.toml')
assert config.isFile()
assert config.text.contains('min_similarity = 0.75')
// build.log holds the last run: the second init, which must refuse to overwrite.
assert log.contains('already exists')
true
