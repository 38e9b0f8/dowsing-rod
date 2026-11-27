import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

// format = "json" came from the file in both executions.
assert report('target/from-file.json').clusters.isEmpty()
assert !report('target/from-pom.json').clusters.isEmpty()
true
