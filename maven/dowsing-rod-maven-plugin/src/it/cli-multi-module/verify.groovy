import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

assert report('target/dowsing-rod.json').functions.size() == 2
assert count(log, 'dowsing-rod report written to') == 1
assert !new File(basedir, 'billing/target/dowsing-rod.json').exists()
true
