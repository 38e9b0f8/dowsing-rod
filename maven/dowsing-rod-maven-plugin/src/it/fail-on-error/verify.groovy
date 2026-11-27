import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

assert log.contains('dowsing-rod failed: parse errors encountered')
assert log.contains('Broken.java')
assert log.contains('(lenient)') && log.contains('(strict)')
true
