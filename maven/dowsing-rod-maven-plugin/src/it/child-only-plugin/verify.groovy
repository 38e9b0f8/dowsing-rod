import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

def json = report('billing/target/dowsing-rod.json')
assert json.functions.size() == 1
assert json.functions[0].function_name == 'charge'
assert !new File(basedir, 'target/dowsing-rod.json').exists()
true
