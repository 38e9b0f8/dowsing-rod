import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

['dowsing-rod:scan', 'dowsing-rod:auto', 'dowsing-rod:init', 'dowsing-rod:check', 'dowsing-rod:cache-clear'].each {
    assert log.contains(it) : it
}
true
