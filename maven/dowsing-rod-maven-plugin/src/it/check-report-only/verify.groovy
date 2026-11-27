import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

assert log.contains('No dowsing-rod thresholds configured')
assert log =~ /C1  java  \w+/
true
