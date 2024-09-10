package instances

import (
	"encoding/json"
	"fmt"
	"io/ioutil"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/ec2"
	log "github.com/sirupsen/logrus"
)

func GetRunningInstances(client *ec2.EC2) (*ec2.DescribeInstancesOutput, error) {
	result, err := client.DescribeInstances(&ec2.DescribeInstancesInput{
		Filters: []*ec2.Filter{
			{
				Name: aws.String("instance-state-name"),
				Values: []*string{
					aws.String("running"),
				},
			},
		},
	})

	if err != nil {
		return nil, err
	}

	return result, err
}

func ListRunningInstances(profile string, region string) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Warn("Failed to initialize new session", err)
		return
	}

	ec2Client := ec2.New(sess)

	runningInstances, err := GetRunningInstances(ec2Client)
	if err != nil {
		log.Warn("Couldn't retrieve running instances", err)
		return
	}

	for _, reservation := range runningInstances.Reservations {
		for _, instance := range reservation.Instances {
			fmt.Println(instance)
		}
	}
}

func GetInstanceDetails(instanceID string, profile string, region string) *ec2.Instance {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session", err)
		return nil
	}

	ec2Client := ec2.New(sess)

	runningInstances, err := GetRunningInstances(ec2Client)
	if err != nil {
		log.Error("Couldn't retrieve running instances", err)
		return nil
	}

	for _, reservation := range runningInstances.Reservations {
		for _, instance := range reservation.Instances {

			if *(instance.InstanceId) == instanceID {
				file, _ := json.MarshalIndent(instance, "", " ")

				_ = ioutil.WriteFile("instance.json", file, 0644)
				return instance
			}

		}

	}
	return nil
}

func GetInstanceFromNameTag(name string, profile string, region string) (bool, *ec2.Instance) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session", err)
		return false, nil
	}

	ec2Client := ec2.New(sess)

	runningInstances, err := GetRunningInstances(ec2Client)
	if err != nil {
		log.Error("Couldn't retrieve running instances", err)
		return false, nil
	}

	for _, reservation := range runningInstances.Reservations {
		for _, instance := range reservation.Instances {
			for _, tagpair := range instance.Tags {
				if *(tagpair.Key) == "Name" && *(tagpair.Value) == name {
					return true, instance
				}
			}
		}

	}
	return false, nil
}

func CheckAMIFromNameTag(amiName string, profile string, region string) bool {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session", err)
		return false
	}

	ec2Client := ec2.New(sess)
	result, err := ec2Client.DescribeImages(&ec2.DescribeImagesInput{
		Owners: []*string{
			aws.String("self"),
		},
		Filters: []*ec2.Filter{
			{
				Name: aws.String("name"),
				Values: []*string{
					aws.String(amiName),
				},
			},
		},
	})
	for _, ami := range result.Images {

		if *ami.Name == amiName {
			return true
		}
	}
	if err != nil {
		log.Error("Couldn't retrieve running instances", err)
		return false
	}

	return false
}

func GetSecurityGroup(client *ec2.EC2) *ec2.SecurityGroup {
	result, err := client.DescribeSecurityGroups(&ec2.DescribeSecurityGroupsInput{
		Filters: []*ec2.Filter{
			&ec2.Filter{
				Name: aws.String("tag:project"),
				Values: []*string{
					aws.String("oyster"),
				},
			},
		},
	})

	if err != nil {
		log.Error("Error fetching security group: ", err)
	}

	for _, group := range result.SecurityGroups {
		for _, tag := range group.Tags {
			if *tag.Key == "project" && *tag.Value == "oyster" {
				return group
			}
		}
	}

	return nil
}

func GetSubnet(client *ec2.EC2) *ec2.Subnet {
	result, err := client.DescribeSubnets(&ec2.DescribeSubnetsInput{
		Filters: []*ec2.Filter{
			&ec2.Filter{
				Name: aws.String("tag:project"),
				Values: []*string{
					aws.String("oyster"),
				},
			},
		},
	})

	if err != nil {
		log.Error("Error fetching subnets: ", err)
	}

	for _, subnet := range result.Subnets {
		for _, tag := range subnet.Tags {
			if *tag.Key == "project" && *tag.Value == "oyster" {
				return subnet
			}
		}
	}

	return nil
}
