# Agate on Debian

If you want to run Agate on a standard Debian install, this directory contains some additional materials that may help you.

## Running Agate as a Service

To run Agate as a service with `systemd`, put the `gemini.service` file in the directory `/etc/systemd/system/` (copy or move it there). This service file has some comments you may want to look at before using it.

If you use the service file and want the Agate logs in a separate file, using the `gemini.conf` file and putting it in the directory `/etc/rsyslog.d/` will make the Agate log messages appear in a file called `/var/log/gemini.log`.

## Rotating Logfiles

If you use Debians `logrotate` and want to automatically rotate these log files, you can use the `geminilogs` file and put it in `/etc/logrotate.d/`.

## Install Script

You can also use the `install.sh` file, which will check if these systems are installed (but not if they are running), and copy the files to their described locations. Please ensure your system's hostname is set correctly (i.e. `uname -n` should give your domain name).

You will have to run this with elevated privileges, i.e. `sudo ./install.sh` to work correctly. This install script will also create the necessary content directories and the certificate and private key in the `/srv/gemini/` directory. After the script is done successfully, you can start by putting content in `/srv/gemini/content/`, the server is running already.
